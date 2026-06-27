use bevy_ecs::prelude::Component;

/// Naivety and credulity tracker. `credulity` accumulates as an entity is
/// deceived via `deceive(amount)` and fades via passive `wisdom_rate` per
/// second in `tick(dt)`. Active enlightenment is available via `enlighten(amount)`.
///
/// Models NPC susceptibility to deception, charm, misdirection, or social
/// engineering. A fully credulous entity is easily fooled; one at 0 is
/// streetwise and resistant.
///
/// `deceive(amount)` adds to credulity (capped at `max_credulity`). Fires
/// `just_fooled` on first reaching max. No-op when disabled.
///
/// `enlighten(amount)` reduces credulity when above 0. Fires `just_savvy`
/// when credulity reaches 0. No-op when disabled.
///
/// `tick(dt)` clears `just_fooled` and `just_savvy`. Then (when enabled
/// and `wisdom_rate > 0`) reduces credulity by `wisdom_rate * dt`, floored
/// at 0. Fires `just_savvy` if credulity reaches 0 via wisdom.
///
/// `is_fooled()` returns `credulity >= max_credulity && enabled`.
///
/// `is_savvy()` returns `credulity == 0.0` (not gated by `enabled`).
///
/// `credulity_fraction()` returns `(credulity / max_credulity).clamp(0, 1)`.
///
/// `effective_vulnerability(base)` returns `base * credulity_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 5.0)` — starts savvy, wisdom drains credulity at 5/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yokel {
    pub credulity: f32,
    pub max_credulity: f32,
    pub wisdom_rate: f32,
    pub just_fooled: bool,
    pub just_savvy: bool,
    pub enabled: bool,
}

impl Yokel {
    pub fn new(max_credulity: f32, wisdom_rate: f32) -> Self {
        Self {
            credulity: 0.0,
            max_credulity: max_credulity.max(0.1),
            wisdom_rate: wisdom_rate.max(0.0),
            just_fooled: false,
            just_savvy: false,
            enabled: true,
        }
    }

    /// Deceive the entity; fires `just_fooled` on first reaching max.
    /// No-op when disabled or already at max.
    pub fn deceive(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.credulity >= self.max_credulity {
            return;
        }
        self.credulity = (self.credulity + amount).min(self.max_credulity);
        if self.credulity >= self.max_credulity {
            self.just_fooled = true;
        }
    }

    /// Enlighten the entity; fires `just_savvy` when credulity reaches 0.
    /// No-op when disabled or already savvy.
    pub fn enlighten(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.credulity <= 0.0 {
            return;
        }
        self.credulity = (self.credulity - amount).max(0.0);
        if self.credulity <= 0.0 {
            self.just_savvy = true;
        }
    }

    /// Advance one frame: clear flags, then reduce credulity via passive
    /// wisdom when enabled and `wisdom_rate > 0`. Fires `just_savvy` if
    /// credulity hits 0.
    pub fn tick(&mut self, dt: f32) {
        self.just_fooled = false;
        self.just_savvy = false;
        if self.enabled && self.wisdom_rate > 0.0 && self.credulity > 0.0 {
            self.credulity = (self.credulity - self.wisdom_rate * dt).max(0.0);
            if self.credulity <= 0.0 {
                self.just_savvy = true;
            }
        }
    }

    /// `true` when credulity is at maximum and component is enabled.
    pub fn is_fooled(&self) -> bool {
        self.credulity >= self.max_credulity && self.enabled
    }

    /// `true` when credulity is 0 (not gated by `enabled`).
    pub fn is_savvy(&self) -> bool {
        self.credulity == 0.0
    }

    /// Fraction of maximum credulity [0.0, 1.0].
    pub fn credulity_fraction(&self) -> f32 {
        (self.credulity / self.max_credulity).clamp(0.0, 1.0)
    }

    /// Returns `base * credulity_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_vulnerability(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.credulity_fraction()
    }
}

impl Default for Yokel {
    fn default() -> Self {
        Self::new(100.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yokel {
        Yokel::new(100.0, 10.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_savvy() {
        let y = y();
        assert_eq!(y.credulity, 0.0);
        assert!(y.is_savvy());
        assert!(!y.is_fooled());
    }

    #[test]
    fn new_clamps_max_credulity() {
        let y = Yokel::new(-5.0, 1.0);
        assert!((y.max_credulity - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_wisdom_rate() {
        let y = Yokel::new(100.0, -3.0);
        assert_eq!(y.wisdom_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let y = Yokel::default();
        assert!((y.max_credulity - 100.0).abs() < 1e-5);
        assert!((y.wisdom_rate - 5.0).abs() < 1e-5);
        assert_eq!(y.credulity, 0.0);
    }

    // --- deceive ---

    #[test]
    fn deceive_increases_credulity() {
        let mut y = y();
        y.deceive(40.0);
        assert!((y.credulity - 40.0).abs() < 1e-4);
    }

    #[test]
    fn deceive_clamps_at_max() {
        let mut y = y();
        y.deceive(200.0);
        assert!((y.credulity - 100.0).abs() < 1e-5);
    }

    #[test]
    fn deceive_fires_just_fooled_at_max() {
        let mut y = y();
        y.deceive(100.0);
        assert!(y.just_fooled);
        assert!(y.is_fooled());
    }

    #[test]
    fn deceive_no_refire_when_at_max() {
        let mut y = y();
        y.deceive(100.0);
        y.deceive(10.0); // already at max
        assert!(y.just_fooled);
    }

    #[test]
    fn deceive_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.deceive(50.0);
        assert_eq!(y.credulity, 0.0);
    }

    #[test]
    fn deceive_no_op_for_zero() {
        let mut y = y();
        y.deceive(0.0);
        assert_eq!(y.credulity, 0.0);
    }

    #[test]
    fn deceive_accumulates() {
        let mut y = y();
        y.deceive(30.0);
        y.deceive(25.0);
        assert!((y.credulity - 55.0).abs() < 1e-3);
    }

    // --- enlighten ---

    #[test]
    fn enlighten_reduces_credulity() {
        let mut y = y();
        y.deceive(70.0);
        y.enlighten(20.0);
        assert!((y.credulity - 50.0).abs() < 1e-3);
    }

    #[test]
    fn enlighten_clamps_at_zero() {
        let mut y = y();
        y.deceive(30.0);
        y.enlighten(200.0);
        assert_eq!(y.credulity, 0.0);
    }

    #[test]
    fn enlighten_fires_just_savvy_at_zero() {
        let mut y = y();
        y.deceive(30.0);
        y.enlighten(30.0);
        assert!(y.just_savvy);
        assert!(y.is_savvy());
    }

    #[test]
    fn enlighten_no_op_when_already_savvy() {
        let mut y = y();
        y.enlighten(10.0); // already 0
        assert!(!y.just_savvy);
    }

    #[test]
    fn enlighten_no_op_when_disabled() {
        let mut y = y();
        y.deceive(50.0);
        y.enabled = false;
        y.enlighten(50.0);
        assert!((y.credulity - 50.0).abs() < 1e-3);
    }

    #[test]
    fn enlighten_no_op_for_zero_amount() {
        let mut y = y();
        y.deceive(50.0);
        y.enlighten(0.0);
        assert!((y.credulity - 50.0).abs() < 1e-3);
    }

    // --- tick (passive wisdom) ---

    #[test]
    fn tick_drains_credulity() {
        let mut y = y(); // wisdom_rate = 10
        y.deceive(60.0);
        y.tick(1.0); // 60 - 10 = 50
        assert!((y.credulity - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clamps_at_zero() {
        let mut y = y();
        y.deceive(5.0);
        y.tick(100.0);
        assert_eq!(y.credulity, 0.0);
    }

    #[test]
    fn tick_fires_just_savvy_on_reaching_zero() {
        let mut y = y();
        y.deceive(5.0);
        y.tick(1.0); // drains 10 → 0
        assert!(y.just_savvy);
    }

    #[test]
    fn tick_no_drain_when_already_savvy() {
        let mut y = y();
        y.tick(100.0); // credulity=0, no change
        assert!(!y.just_savvy);
    }

    #[test]
    fn tick_no_drain_when_rate_zero() {
        let mut y = Yokel::new(100.0, 0.0);
        y.deceive(50.0);
        y.tick(100.0);
        assert!((y.credulity - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_drain_when_disabled() {
        let mut y = y();
        y.deceive(50.0);
        y.enabled = false;
        y.tick(1.0);
        assert!((y.credulity - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_fooled() {
        let mut y = y();
        y.deceive(100.0);
        y.tick(0.016);
        assert!(!y.just_fooled);
    }

    #[test]
    fn tick_clears_just_savvy() {
        let mut y = y();
        y.deceive(5.0);
        y.tick(1.0); // just_savvy fires
        y.tick(0.016); // cleared
        assert!(!y.just_savvy);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut y = y();
        y.deceive(80.0);
        y.tick(2.0); // 80 - 10*2 = 60
        assert!((y.credulity - 60.0).abs() < 1e-2);
    }

    // --- is_fooled / is_savvy ---

    #[test]
    fn is_fooled_false_below_max() {
        let mut y = y();
        y.deceive(50.0);
        assert!(!y.is_fooled());
    }

    #[test]
    fn is_fooled_false_when_disabled() {
        let mut y = y();
        y.deceive(100.0);
        y.enabled = false;
        assert!(!y.is_fooled());
    }

    #[test]
    fn is_savvy_true_at_start() {
        assert!(y().is_savvy());
    }

    #[test]
    fn is_savvy_not_gated_by_enabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_savvy());
    }

    // --- fractions / effective ---

    #[test]
    fn credulity_fraction_zero_when_savvy() {
        assert_eq!(y().credulity_fraction(), 0.0);
    }

    #[test]
    fn credulity_fraction_half_at_midpoint() {
        let mut y = y();
        y.deceive(50.0);
        assert!((y.credulity_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_vulnerability_zero_when_savvy() {
        assert_eq!(y().effective_vulnerability(100.0), 0.0);
    }

    #[test]
    fn effective_vulnerability_scales_with_fraction() {
        let mut y = y();
        y.deceive(75.0);
        assert!((y.effective_vulnerability(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_vulnerability_zero_when_disabled() {
        let mut y = y();
        y.deceive(50.0);
        y.enabled = false;
        assert_eq!(y.effective_vulnerability(100.0), 0.0);
    }
}
