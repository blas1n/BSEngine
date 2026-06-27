use bevy_ecs::prelude::Component;

/// Structural fragility escalator. Tracks cumulative damage to an entity's
/// core — as `fragility` approaches `max_fragility`, subsequent hits deal up
/// to `damage_multiplier`× more damage (via `effective_damage_mult()`). Models
/// cracking armor, deteriorating materials, increasing vulnerability, or
/// break-before-break mechanics.
///
/// `crack(amount)` adds fragility when enabled. Fires `just_cracked` the first
/// time fragility reaches `max_fragility`. No-op when disabled.
///
/// `repair(amount)` reduces fragility when enabled. No-op when disabled.
///
/// `tick(_dt)` clears `just_cracked` only.
///
/// `is_cracked()` returns `fragility >= max_fragility && enabled`.
///
/// `fragility_fraction()` returns `(fragility / max_fragility).clamp(0.0, 1.0)`.
///
/// `effective_damage_mult()` returns a value linearly interpolated from `1.0`
/// (at 0 fragility) to `damage_multiplier` (at max fragility) when enabled;
/// `1.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — max 100 fragility, doubles damage when cracked.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yolk {
    /// Accumulated structural damage. Clamped to [0, max_fragility].
    pub fragility: f32,
    /// Fragility level that triggers cracking. Clamped >= 0.1.
    pub max_fragility: f32,
    /// Damage multiplier applied when fully cracked. Clamped >= 1.0.
    pub damage_multiplier: f32,
    pub just_cracked: bool,
    pub enabled: bool,
}

impl Yolk {
    pub fn new(max_fragility: f32, damage_multiplier: f32) -> Self {
        Self {
            fragility: 0.0,
            max_fragility: max_fragility.max(0.1),
            damage_multiplier: damage_multiplier.max(1.0),
            just_cracked: false,
            enabled: true,
        }
    }

    /// Add fragility. Fires `just_cracked` on first reaching maximum. No-op
    /// when disabled or already cracked.
    pub fn crack(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.fragility >= self.max_fragility {
            return;
        }
        self.fragility = (self.fragility + amount).min(self.max_fragility);
        if self.fragility >= self.max_fragility {
            self.just_cracked = true;
        }
    }

    /// Reduce fragility toward 0. No-op when disabled.
    pub fn repair(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        self.fragility = (self.fragility - amount).max(0.0);
    }

    /// Advance one frame: clear `just_cracked` only.
    pub fn tick(&mut self, _dt: f32) {
        self.just_cracked = false;
    }

    /// `true` when fragility has reached maximum and component is enabled.
    pub fn is_cracked(&self) -> bool {
        self.fragility >= self.max_fragility && self.enabled
    }

    /// Fragility as a fraction of maximum [0.0, 1.0].
    pub fn fragility_fraction(&self) -> f32 {
        if self.max_fragility <= 0.0 {
            return 0.0;
        }
        (self.fragility / self.max_fragility).clamp(0.0, 1.0)
    }

    /// Damage multiplier scaled from `1.0` (intact) to `damage_multiplier`
    /// (fully cracked) when enabled; `1.0` when disabled.
    pub fn effective_damage_mult(&self) -> f32 {
        if !self.enabled {
            return 1.0;
        }
        let f = self.fragility_fraction();
        1.0 + (self.damage_multiplier - 1.0) * f
    }
}

impl Default for Yolk {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yolk {
        Yolk::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_intact() {
        let y = y();
        assert_eq!(y.fragility, 0.0);
        assert!(!y.just_cracked);
        assert!(!y.is_cracked());
    }

    #[test]
    fn max_fragility_clamped_to_point_one() {
        let y = Yolk::new(-5.0, 2.0);
        assert!((y.max_fragility - 0.1).abs() < 1e-5);
    }

    #[test]
    fn damage_multiplier_clamped_to_one() {
        let y = Yolk::new(100.0, 0.5);
        assert!((y.damage_multiplier - 1.0).abs() < 1e-5);
    }

    #[test]
    fn default_values() {
        let y = Yolk::default();
        assert!((y.max_fragility - 100.0).abs() < 1e-5);
        assert!((y.damage_multiplier - 2.0).abs() < 1e-5);
    }

    // --- crack ---

    #[test]
    fn crack_adds_fragility() {
        let mut y = y();
        y.crack(30.0);
        assert!((y.fragility - 30.0).abs() < 1e-4);
    }

    #[test]
    fn crack_clamps_at_max() {
        let mut y = y();
        y.crack(999.0);
        assert!((y.fragility - 100.0).abs() < 1e-5);
    }

    #[test]
    fn crack_fires_just_cracked_when_crossing_max() {
        let mut y = y();
        y.crack(100.0);
        assert!(y.just_cracked);
        assert!(y.is_cracked());
    }

    #[test]
    fn crack_does_not_refire_when_already_cracked() {
        let mut y = y();
        y.crack(100.0);
        y.tick(0.016);
        y.crack(10.0); // already at max — no-op
        assert!(!y.just_cracked);
    }

    #[test]
    fn crack_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.crack(50.0);
        assert_eq!(y.fragility, 0.0);
        assert!(!y.just_cracked);
    }

    #[test]
    fn crack_no_op_for_zero_amount() {
        let mut y = y();
        y.crack(0.0);
        assert_eq!(y.fragility, 0.0);
    }

    #[test]
    fn crack_no_op_for_negative_amount() {
        let mut y = y();
        y.crack(-10.0);
        assert_eq!(y.fragility, 0.0);
    }

    // --- repair ---

    #[test]
    fn repair_reduces_fragility() {
        let mut y = y();
        y.crack(60.0);
        y.repair(20.0);
        assert!((y.fragility - 40.0).abs() < 1e-4);
    }

    #[test]
    fn repair_clamps_at_zero() {
        let mut y = y();
        y.crack(30.0);
        y.repair(999.0);
        assert_eq!(y.fragility, 0.0);
    }

    #[test]
    fn repair_no_op_when_disabled() {
        let mut y = y();
        y.crack(50.0);
        y.enabled = false;
        y.repair(20.0);
        assert!((y.fragility - 50.0).abs() < 1e-5);
    }

    #[test]
    fn repair_no_op_for_zero_amount() {
        let mut y = y();
        y.crack(50.0);
        y.repair(0.0);
        assert!((y.fragility - 50.0).abs() < 1e-5);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_cracked() {
        let mut y = y();
        y.crack(100.0);
        y.tick(0.016);
        assert!(!y.just_cracked);
    }

    #[test]
    fn tick_does_not_change_fragility() {
        let mut y = y();
        y.crack(50.0);
        y.tick(1000.0);
        assert!((y.fragility - 50.0).abs() < 1e-5);
    }

    // --- is_cracked ---

    #[test]
    fn is_cracked_false_below_max() {
        let mut y = y();
        y.crack(99.9);
        assert!(!y.is_cracked());
    }

    #[test]
    fn is_cracked_true_at_max() {
        let mut y = y();
        y.crack(100.0);
        assert!(y.is_cracked());
    }

    #[test]
    fn is_cracked_false_when_disabled() {
        let mut y = y();
        y.crack(100.0);
        y.enabled = false;
        assert!(!y.is_cracked());
    }

    // --- fragility_fraction ---

    #[test]
    fn fragility_fraction_zero_when_intact() {
        assert_eq!(y().fragility_fraction(), 0.0);
    }

    #[test]
    fn fragility_fraction_one_when_fully_cracked() {
        let mut y = y();
        y.crack(100.0);
        assert!((y.fragility_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn fragility_fraction_half_at_midpoint() {
        let mut y = y();
        y.crack(50.0);
        assert!((y.fragility_fraction() - 0.5).abs() < 1e-4);
    }

    // --- effective_damage_mult ---

    #[test]
    fn effective_damage_mult_one_when_intact() {
        assert!((y().effective_damage_mult() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_damage_mult_max_when_fully_cracked() {
        let mut y = y(); // mult=2.0
        y.crack(100.0);
        assert!((y.effective_damage_mult() - 2.0).abs() < 1e-4);
    }

    #[test]
    fn effective_damage_mult_midpoint_at_half_fragility() {
        let mut y = y(); // mult=2.0, so midpoint=1.5
        y.crack(50.0);
        assert!((y.effective_damage_mult() - 1.5).abs() < 1e-4);
    }

    #[test]
    fn effective_damage_mult_one_when_disabled() {
        let mut y = y();
        y.crack(100.0);
        y.enabled = false;
        assert!((y.effective_damage_mult() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_damage_mult_one_when_multiplier_is_one() {
        let mut y = Yolk::new(100.0, 1.0); // no bonus
        y.crack(100.0);
        assert!((y.effective_damage_mult() - 1.0).abs() < 1e-5);
    }

    // --- crack-repair cycle ---

    #[test]
    fn repair_after_crack_allows_re_cracking() {
        let mut y = y();
        y.crack(100.0);
        y.tick(0.016);
        y.repair(100.0);
        y.crack(100.0);
        assert!(y.just_cracked);
    }
}
