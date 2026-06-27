use bevy_ecs::prelude::Component;

/// Affirmation and consent tracker. `consent` accumulates via `affirm(amount)`
/// and fades via passive `doubt_rate` per second in `tick(dt)`. Active
/// withdrawal is available via `retract(amount)`.
///
/// Models NPC willingness to comply, quest-acceptance thresholds, diplomatic
/// agreement meters, vote tallies, or any mechanic where an entity's agreement
/// must be earned and can erode through inaction or opposition.
///
/// `affirm(amount)` adds to consent (capped at `max_consent`). Fires
/// `just_agreed` on first reaching max. No-op when disabled.
///
/// `retract(amount)` reduces consent when above 0. Fires `just_withdrew`
/// when consent reaches 0. No-op when disabled.
///
/// `tick(dt)` clears `just_agreed` and `just_withdrew`. Then (when enabled
/// and `doubt_rate > 0`) reduces consent by `doubt_rate * dt`, floored at 0.
/// Fires `just_withdrew` if consent reaches 0 via doubt.
///
/// `is_agreed()` returns `consent >= max_consent && enabled`.
///
/// `is_withdrawn()` returns `consent == 0.0` (not gated by `enabled`).
///
/// `consent_fraction()` returns `(consent / max_consent).clamp(0, 1)`.
///
/// `effective_compliance(base)` returns `base * consent_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 0.0)` — starts uncommitted, no passive doubt.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yep {
    pub consent: f32,
    pub max_consent: f32,
    pub doubt_rate: f32,
    pub just_agreed: bool,
    pub just_withdrew: bool,
    pub enabled: bool,
}

impl Yep {
    pub fn new(max_consent: f32, doubt_rate: f32) -> Self {
        Self {
            consent: 0.0,
            max_consent: max_consent.max(0.1),
            doubt_rate: doubt_rate.max(0.0),
            just_agreed: false,
            just_withdrew: false,
            enabled: true,
        }
    }

    /// Build consent; fires `just_agreed` on first reaching max.
    /// No-op when disabled or already at max.
    pub fn affirm(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.consent >= self.max_consent {
            return;
        }
        self.consent = (self.consent + amount).min(self.max_consent);
        if self.consent >= self.max_consent {
            self.just_agreed = true;
        }
    }

    /// Withdraw consent; fires `just_withdrew` when reaching 0.
    /// No-op when disabled or already withdrawn.
    pub fn retract(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.consent <= 0.0 {
            return;
        }
        self.consent = (self.consent - amount).max(0.0);
        if self.consent <= 0.0 {
            self.just_withdrew = true;
        }
    }

    /// Advance one frame: clear flags, then reduce consent via passive doubt
    /// when enabled and `doubt_rate > 0`. Fires `just_withdrew` if consent
    /// reaches 0.
    pub fn tick(&mut self, dt: f32) {
        self.just_agreed = false;
        self.just_withdrew = false;
        if self.enabled && self.doubt_rate > 0.0 && self.consent > 0.0 {
            self.consent = (self.consent - self.doubt_rate * dt).max(0.0);
            if self.consent <= 0.0 {
                self.just_withdrew = true;
            }
        }
    }

    /// `true` when consent is at maximum and component is enabled.
    pub fn is_agreed(&self) -> bool {
        self.consent >= self.max_consent && self.enabled
    }

    /// `true` when consent is 0 (not gated by `enabled`).
    pub fn is_withdrawn(&self) -> bool {
        self.consent == 0.0
    }

    /// Fraction of maximum consent [0.0, 1.0].
    pub fn consent_fraction(&self) -> f32 {
        (self.consent / self.max_consent).clamp(0.0, 1.0)
    }

    /// Returns `base * consent_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_compliance(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.consent_fraction()
    }
}

impl Default for Yep {
    fn default() -> Self {
        Self::new(100.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yep {
        Yep::new(100.0, 10.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_withdrawn() {
        let y = y();
        assert_eq!(y.consent, 0.0);
        assert!(y.is_withdrawn());
        assert!(!y.is_agreed());
    }

    #[test]
    fn new_clamps_max_consent() {
        let y = Yep::new(-5.0, 0.0);
        assert!((y.max_consent - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_doubt_rate() {
        let y = Yep::new(100.0, -3.0);
        assert_eq!(y.doubt_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let y = Yep::default();
        assert!((y.max_consent - 100.0).abs() < 1e-5);
        assert_eq!(y.doubt_rate, 0.0);
        assert_eq!(y.consent, 0.0);
    }

    // --- affirm ---

    #[test]
    fn affirm_increases_consent() {
        let mut y = y();
        y.affirm(40.0);
        assert!((y.consent - 40.0).abs() < 1e-4);
    }

    #[test]
    fn affirm_clamps_at_max() {
        let mut y = y();
        y.affirm(200.0);
        assert!((y.consent - 100.0).abs() < 1e-5);
    }

    #[test]
    fn affirm_fires_just_agreed_at_max() {
        let mut y = y();
        y.affirm(100.0);
        assert!(y.just_agreed);
        assert!(y.is_agreed());
    }

    #[test]
    fn affirm_no_refire_when_at_max() {
        let mut y = y();
        y.affirm(100.0);
        y.affirm(10.0); // already at max
        assert!(y.just_agreed);
    }

    #[test]
    fn affirm_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.affirm(50.0);
        assert_eq!(y.consent, 0.0);
    }

    #[test]
    fn affirm_no_op_for_zero() {
        let mut y = y();
        y.affirm(0.0);
        assert_eq!(y.consent, 0.0);
    }

    #[test]
    fn affirm_accumulates() {
        let mut y = y();
        y.affirm(30.0);
        y.affirm(25.0);
        assert!((y.consent - 55.0).abs() < 1e-3);
    }

    // --- retract ---

    #[test]
    fn retract_reduces_consent() {
        let mut y = y();
        y.affirm(70.0);
        y.retract(20.0);
        assert!((y.consent - 50.0).abs() < 1e-3);
    }

    #[test]
    fn retract_clamps_at_zero() {
        let mut y = y();
        y.affirm(30.0);
        y.retract(200.0);
        assert_eq!(y.consent, 0.0);
    }

    #[test]
    fn retract_fires_just_withdrew_at_zero() {
        let mut y = y();
        y.affirm(30.0);
        y.retract(30.0);
        assert!(y.just_withdrew);
        assert!(y.is_withdrawn());
    }

    #[test]
    fn retract_no_op_when_already_withdrawn() {
        let mut y = y();
        y.retract(10.0); // already 0
        assert!(!y.just_withdrew);
    }

    #[test]
    fn retract_no_op_when_disabled() {
        let mut y = y();
        y.affirm(50.0);
        y.enabled = false;
        y.retract(50.0);
        assert!((y.consent - 50.0).abs() < 1e-3);
    }

    #[test]
    fn retract_no_op_for_zero_amount() {
        let mut y = y();
        y.affirm(50.0);
        y.retract(0.0);
        assert!((y.consent - 50.0).abs() < 1e-3);
    }

    // --- tick (passive doubt) ---

    #[test]
    fn tick_erodes_consent() {
        let mut y = y(); // doubt_rate = 10
        y.affirm(60.0);
        y.tick(1.0); // 60 - 10 = 50
        assert!((y.consent - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clamps_at_zero() {
        let mut y = y();
        y.affirm(5.0);
        y.tick(100.0);
        assert_eq!(y.consent, 0.0);
    }

    #[test]
    fn tick_fires_just_withdrew_on_reaching_zero() {
        let mut y = y();
        y.affirm(5.0);
        y.tick(1.0); // drains 10 → 0
        assert!(y.just_withdrew);
    }

    #[test]
    fn tick_no_erosion_when_withdrawn() {
        let mut y = y();
        y.tick(100.0); // consent=0
        assert!(!y.just_withdrew);
    }

    #[test]
    fn tick_no_erosion_when_rate_zero() {
        let mut y = Yep::new(100.0, 0.0);
        y.affirm(50.0);
        y.tick(100.0);
        assert!((y.consent - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_erosion_when_disabled() {
        let mut y = y();
        y.affirm(50.0);
        y.enabled = false;
        y.tick(1.0);
        assert!((y.consent - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_agreed() {
        let mut y = y();
        y.affirm(100.0);
        y.tick(0.016);
        assert!(!y.just_agreed);
    }

    #[test]
    fn tick_clears_just_withdrew() {
        let mut y = y();
        y.affirm(5.0);
        y.tick(1.0); // just_withdrew fires
        y.tick(0.016); // cleared
        assert!(!y.just_withdrew);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut y = y();
        y.affirm(80.0);
        y.tick(2.0); // 80 - 10*2 = 60
        assert!((y.consent - 60.0).abs() < 1e-2);
    }

    // --- is_agreed / is_withdrawn ---

    #[test]
    fn is_agreed_false_below_max() {
        let mut y = y();
        y.affirm(50.0);
        assert!(!y.is_agreed());
    }

    #[test]
    fn is_agreed_false_when_disabled() {
        let mut y = y();
        y.affirm(100.0);
        y.enabled = false;
        assert!(!y.is_agreed());
    }

    #[test]
    fn is_withdrawn_true_at_start() {
        assert!(y().is_withdrawn());
    }

    #[test]
    fn is_withdrawn_not_gated_by_enabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_withdrawn());
    }

    // --- fractions / effective ---

    #[test]
    fn consent_fraction_zero_when_withdrawn() {
        assert_eq!(y().consent_fraction(), 0.0);
    }

    #[test]
    fn consent_fraction_half_at_midpoint() {
        let mut y = y();
        y.affirm(50.0);
        assert!((y.consent_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_compliance_zero_when_withdrawn() {
        assert_eq!(y().effective_compliance(100.0), 0.0);
    }

    #[test]
    fn effective_compliance_scales_with_fraction() {
        let mut y = y();
        y.affirm(75.0);
        assert!((y.effective_compliance(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_compliance_zero_when_disabled() {
        let mut y = y();
        y.affirm(50.0);
        y.enabled = false;
        assert_eq!(y.effective_compliance(100.0), 0.0);
    }
}
