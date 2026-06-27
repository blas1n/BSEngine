use bevy_ecs::prelude::Component;

/// Age-stage tracker with passive advancement and renewal. Tracks how far
/// through a lifecycle an entity has progressed. `age` advances toward
/// `max_age` via `age_by()` or passively through `tick(dt)` when
/// `aging_rate > 0`. Fires `just_matured` on reaching max. `renew()` resets
/// age to 0 and fires `just_renewed`.
///
/// Models character aging, ripening timers, spell duration stages, freshness
/// meters, or any lifecycle where an entity starts young and matures over time.
///
/// `age_by(amount)` adds to `age` (clamped to `max_age`). Fires
/// `just_matured` on first reaching max. No-op when disabled or already mature.
///
/// `renew()` resets `age` to 0 and fires `just_renewed`. No-op when disabled
/// or already at 0.
///
/// `tick(dt)` clears `just_matured` and `just_renewed`, then (when enabled
/// and `aging_rate > 0`) calls `age_by(aging_rate * dt)`.
///
/// `is_mature()` returns `age >= max_age && enabled`.
///
/// `is_fresh()` returns `age == 0.0` (not gated by `enabled`).
///
/// `youth_fraction()` returns `1.0 - (age / max_age).clamp(0, 1)` — 1.0
/// when fresh, 0.0 when fully mature.
///
/// `effective_vitality(base)` returns `base * youth_fraction()` when enabled;
/// `0.0` when disabled. Models vigor that wanes with age.
///
/// Default: `new(100.0, 0.0)` — no passive aging, starts fresh.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Youth {
    pub age: f32,
    pub max_age: f32,
    pub aging_rate: f32,
    pub just_matured: bool,
    pub just_renewed: bool,
    pub enabled: bool,
}

impl Youth {
    pub fn new(max_age: f32, aging_rate: f32) -> Self {
        Self {
            age: 0.0,
            max_age: max_age.max(0.1),
            aging_rate: aging_rate.max(0.0),
            just_matured: false,
            just_renewed: false,
            enabled: true,
        }
    }

    /// Advance age. Fires `just_matured` on first reaching `max_age`. No-op
    /// when disabled or already mature.
    pub fn age_by(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.age >= self.max_age {
            return;
        }
        self.age = (self.age + amount).min(self.max_age);
        if self.age >= self.max_age {
            self.just_matured = true;
        }
    }

    /// Reset age to 0. Fires `just_renewed`. No-op when disabled or already
    /// fresh.
    pub fn renew(&mut self) {
        if !self.enabled || self.age == 0.0 {
            return;
        }
        self.age = 0.0;
        self.just_renewed = true;
    }

    /// Advance one frame: clear flags, then age passively when enabled and
    /// `aging_rate > 0`.
    pub fn tick(&mut self, dt: f32) {
        self.just_matured = false;
        self.just_renewed = false;
        if self.enabled && self.aging_rate > 0.0 {
            self.age_by(self.aging_rate * dt);
        }
    }

    /// `true` when fully mature and component is enabled.
    pub fn is_mature(&self) -> bool {
        self.age >= self.max_age && self.enabled
    }

    /// `true` when age is 0 (not gated by `enabled`).
    pub fn is_fresh(&self) -> bool {
        self.age == 0.0
    }

    /// Fraction of youthfulness remaining [0.0, 1.0]. 1.0 = fully fresh,
    /// 0.0 = fully mature.
    pub fn youth_fraction(&self) -> f32 {
        1.0 - (self.age / self.max_age).clamp(0.0, 1.0)
    }

    /// Returns `base * youth_fraction()` when enabled; `0.0` when disabled.
    /// Represents vigor or power that wanes as the entity ages.
    pub fn effective_vitality(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.youth_fraction()
    }
}

impl Default for Youth {
    fn default() -> Self {
        Self::new(100.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Youth {
        Youth::new(100.0, 0.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_fresh() {
        let y = y();
        assert_eq!(y.age, 0.0);
        assert!(y.is_fresh());
        assert!(!y.is_mature());
    }

    #[test]
    fn new_clamps_max_age() {
        let y = Youth::new(-5.0, 0.0);
        assert!((y.max_age - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_aging_rate() {
        let y = Youth::new(100.0, -2.0);
        assert_eq!(y.aging_rate, 0.0);
    }

    #[test]
    fn default_max_age_is_hundred() {
        assert!((Youth::default().max_age - 100.0).abs() < 1e-5);
    }

    // --- age_by ---

    #[test]
    fn age_by_increases_age() {
        let mut y = y();
        y.age_by(30.0);
        assert!((y.age - 30.0).abs() < 1e-4);
    }

    #[test]
    fn age_by_clamps_at_max() {
        let mut y = y();
        y.age_by(200.0);
        assert!((y.age - 100.0).abs() < 1e-5);
    }

    #[test]
    fn age_by_fires_just_matured() {
        let mut y = y();
        y.age_by(100.0);
        assert!(y.just_matured);
        assert!(y.is_mature());
    }

    #[test]
    fn age_by_no_refire_when_already_mature() {
        let mut y = y();
        y.age_by(100.0);
        y.tick(0.016);
        y.age_by(10.0); // already at max
        assert!(!y.just_matured);
    }

    #[test]
    fn age_by_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.age_by(50.0);
        assert_eq!(y.age, 0.0);
    }

    #[test]
    fn age_by_no_op_for_zero_amount() {
        let mut y = y();
        y.age_by(0.0);
        assert_eq!(y.age, 0.0);
    }

    // --- renew ---

    #[test]
    fn renew_resets_age_to_zero() {
        let mut y = y();
        y.age_by(60.0);
        y.tick(0.016);
        y.renew();
        assert_eq!(y.age, 0.0);
        assert!(y.is_fresh());
    }

    #[test]
    fn renew_fires_just_renewed() {
        let mut y = y();
        y.age_by(30.0);
        y.tick(0.016);
        y.renew();
        assert!(y.just_renewed);
    }

    #[test]
    fn renew_no_op_when_already_fresh() {
        let mut y = y();
        y.renew();
        assert!(!y.just_renewed);
    }

    #[test]
    fn renew_no_op_when_disabled() {
        let mut y = y();
        y.age_by(50.0);
        y.enabled = false;
        y.renew();
        assert!((y.age - 50.0).abs() < 1e-4);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_matured() {
        let mut y = y();
        y.age_by(100.0);
        y.tick(0.016);
        assert!(!y.just_matured);
    }

    #[test]
    fn tick_clears_just_renewed() {
        let mut y = y();
        y.age_by(30.0);
        y.renew();
        y.tick(0.016);
        assert!(!y.just_renewed);
    }

    #[test]
    fn tick_ages_passively_with_rate() {
        let mut y = Youth::new(100.0, 10.0);
        y.tick(1.0); // 10 * 1 = 10
        assert!((y.age - 10.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_aging_when_rate_zero() {
        let mut y = y();
        y.tick(100.0);
        assert_eq!(y.age, 0.0);
    }

    #[test]
    fn tick_fires_just_matured_via_passive_aging() {
        let mut y = Youth::new(10.0, 100.0);
        y.tick(1.0); // 100 * 1 >> 10
        assert!(y.just_matured);
    }

    #[test]
    fn tick_no_aging_when_disabled() {
        let mut y = Youth::new(100.0, 10.0);
        y.enabled = false;
        y.tick(1.0);
        assert_eq!(y.age, 0.0);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut y = Youth::new(100.0, 20.0);
        y.tick(0.5); // 20 * 0.5 = 10
        assert!((y.age - 10.0).abs() < 1e-3);
    }

    // --- is_mature / is_fresh ---

    #[test]
    fn is_mature_false_below_max() {
        let mut y = y();
        y.age_by(50.0);
        assert!(!y.is_mature());
    }

    #[test]
    fn is_mature_false_when_disabled() {
        let mut y = y();
        y.age_by(100.0);
        y.enabled = false;
        assert!(!y.is_mature());
    }

    #[test]
    fn is_fresh_true_at_zero() {
        assert!(y().is_fresh());
    }

    #[test]
    fn is_fresh_true_even_when_disabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_fresh()); // not gated
    }

    #[test]
    fn is_fresh_false_with_any_age() {
        let mut y = y();
        y.age_by(0.001);
        assert!(!y.is_fresh());
    }

    // --- youth_fraction / effective_vitality ---

    #[test]
    fn youth_fraction_one_when_fresh() {
        assert!((y().youth_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn youth_fraction_half_at_midpoint() {
        let mut y = y();
        y.age_by(50.0);
        assert!((y.youth_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn youth_fraction_zero_when_mature() {
        let mut y = y();
        y.age_by(100.0);
        assert!((y.youth_fraction() - 0.0).abs() < 1e-5);
    }

    #[test]
    fn effective_vitality_full_when_fresh() {
        assert!((y().effective_vitality(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_vitality_scales_with_fraction() {
        let mut y = y();
        y.age_by(25.0);
        assert!((y.effective_vitality(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_vitality_zero_when_mature() {
        let mut y = y();
        y.age_by(100.0);
        assert_eq!(y.effective_vitality(100.0), 0.0);
    }

    #[test]
    fn effective_vitality_zero_when_disabled() {
        let mut y = y();
        y.enabled = false;
        assert_eq!(y.effective_vitality(100.0), 0.0);
    }
}
