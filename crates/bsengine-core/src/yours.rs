use bevy_ecs::prelude::Component;

/// Ownership-claim tracker for territory and resource contention. `claim`
/// in [0, max_claim] represents how strongly this entity owns a contested
/// resource. `seize(amount)` asserts ownership; `contest(amount)` challenges
/// it; `tick(dt)` erodes claim passively at `erosion_rate` per second to
/// model unchecked contestation.
///
/// Models territory capture points, resource disputes, ownership races,
/// or any mechanic where claim is built up by presence and eroded by
/// challengers or neglect.
///
/// `seize(amount)` adds to claim when below max. Fires `just_claimed` on
/// first reaching `max_claim`. No-op when disabled.
///
/// `contest(amount)` reduces claim when above 0. Fires `just_lost` when
/// claim first reaches 0. No-op when disabled.
///
/// `tick(dt)` clears `just_claimed` and `just_lost`. Then (when enabled
/// and `erosion_rate > 0`) reduces claim by `erosion_rate * dt`. Fires
/// `just_lost` if claim reaches 0 via erosion.
///
/// `is_claimed()` returns `claim >= max_claim && enabled`.
///
/// `is_lost()` returns `claim == 0.0` (not gated by `enabled`).
///
/// `claim_fraction()` returns `(claim / max_claim).clamp(0, 1)`.
///
/// `effective_control(base)` returns `base * claim_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 0.0)` — starts unclaimed, no passive erosion.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yours {
    pub claim: f32,
    pub max_claim: f32,
    pub erosion_rate: f32,
    pub just_claimed: bool,
    pub just_lost: bool,
    pub enabled: bool,
}

impl Yours {
    pub fn new(max_claim: f32, erosion_rate: f32) -> Self {
        Self {
            claim: 0.0,
            max_claim: max_claim.max(0.1),
            erosion_rate: erosion_rate.max(0.0),
            just_claimed: false,
            just_lost: false,
            enabled: true,
        }
    }

    /// Assert ownership; fires `just_claimed` on first reaching max.
    /// No-op when disabled or already at max.
    pub fn seize(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.claim >= self.max_claim {
            return;
        }
        self.claim = (self.claim + amount).min(self.max_claim);
        if self.claim >= self.max_claim {
            self.just_claimed = true;
        }
    }

    /// Challenge the claim; fires `just_lost` when reaching 0.
    /// No-op when disabled or claim already 0.
    pub fn contest(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.claim <= 0.0 {
            return;
        }
        self.claim = (self.claim - amount).max(0.0);
        if self.claim <= 0.0 {
            self.just_lost = true;
        }
    }

    /// Advance one frame: clear flags, then erode claim passively when
    /// enabled and `erosion_rate > 0`. Fires `just_lost` if claim reaches 0.
    pub fn tick(&mut self, dt: f32) {
        self.just_claimed = false;
        self.just_lost = false;
        if self.enabled && self.erosion_rate > 0.0 && self.claim > 0.0 {
            self.claim = (self.claim - self.erosion_rate * dt).max(0.0);
            if self.claim <= 0.0 {
                self.just_lost = true;
            }
        }
    }

    /// `true` when claim is at maximum and component is enabled.
    pub fn is_claimed(&self) -> bool {
        self.claim >= self.max_claim && self.enabled
    }

    /// `true` when claim is 0 (not gated by `enabled`).
    pub fn is_lost(&self) -> bool {
        self.claim == 0.0
    }

    /// Fraction of maximum claim [0.0, 1.0].
    pub fn claim_fraction(&self) -> f32 {
        (self.claim / self.max_claim).clamp(0.0, 1.0)
    }

    /// Returns `base * claim_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_control(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.claim_fraction()
    }
}

impl Default for Yours {
    fn default() -> Self {
        Self::new(100.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yours {
        Yours::new(100.0, 0.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_unclaimed() {
        let y = y();
        assert_eq!(y.claim, 0.0);
        assert!(y.is_lost());
        assert!(!y.is_claimed());
    }

    #[test]
    fn new_clamps_max_claim() {
        let y = Yours::new(-5.0, 0.0);
        assert!((y.max_claim - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_erosion_rate() {
        let y = Yours::new(100.0, -5.0);
        assert_eq!(y.erosion_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let y = Yours::default();
        assert!((y.max_claim - 100.0).abs() < 1e-5);
        assert_eq!(y.erosion_rate, 0.0);
        assert_eq!(y.claim, 0.0);
    }

    // --- seize ---

    #[test]
    fn seize_increases_claim() {
        let mut y = y();
        y.seize(40.0);
        assert!((y.claim - 40.0).abs() < 1e-4);
    }

    #[test]
    fn seize_clamps_at_max() {
        let mut y = y();
        y.seize(200.0);
        assert!((y.claim - 100.0).abs() < 1e-5);
    }

    #[test]
    fn seize_fires_just_claimed_at_max() {
        let mut y = y();
        y.seize(100.0);
        assert!(y.just_claimed);
        assert!(y.is_claimed());
    }

    #[test]
    fn seize_no_refire_when_at_max() {
        let mut y = y();
        y.seize(100.0); // just_claimed=true
        y.seize(10.0); // already at max
        assert!(y.just_claimed);
    }

    #[test]
    fn seize_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.seize(50.0);
        assert_eq!(y.claim, 0.0);
    }

    #[test]
    fn seize_no_op_for_zero_amount() {
        let mut y = y();
        y.seize(0.0);
        assert_eq!(y.claim, 0.0);
    }

    #[test]
    fn seize_accumulates() {
        let mut y = y();
        y.seize(30.0);
        y.seize(30.0);
        assert!((y.claim - 60.0).abs() < 1e-3);
    }

    // --- contest ---

    #[test]
    fn contest_reduces_claim() {
        let mut y = y();
        y.seize(70.0);
        y.contest(20.0);
        assert!((y.claim - 50.0).abs() < 1e-3);
    }

    #[test]
    fn contest_clamps_at_zero() {
        let mut y = y();
        y.seize(30.0);
        y.contest(200.0);
        assert_eq!(y.claim, 0.0);
    }

    #[test]
    fn contest_fires_just_lost_at_zero() {
        let mut y = y();
        y.seize(30.0);
        y.contest(30.0);
        assert!(y.just_lost);
        assert!(y.is_lost());
    }

    #[test]
    fn contest_no_op_when_already_lost() {
        let mut y = y();
        y.contest(10.0); // already 0
        assert!(!y.just_lost);
    }

    #[test]
    fn contest_no_op_when_disabled() {
        let mut y = y();
        y.seize(50.0);
        y.enabled = false;
        y.contest(50.0);
        assert!((y.claim - 50.0).abs() < 1e-3);
    }

    #[test]
    fn contest_no_op_for_zero_amount() {
        let mut y = y();
        y.seize(50.0);
        y.contest(0.0);
        assert!((y.claim - 50.0).abs() < 1e-3);
    }

    // --- tick (passive erosion) ---

    #[test]
    fn tick_erodes_claim() {
        let mut y = Yours::new(100.0, 10.0);
        y.seize(60.0);
        y.tick(1.0); // 60 - 10 = 50
        assert!((y.claim - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clamps_erosion_at_zero() {
        let mut y = Yours::new(100.0, 10.0);
        y.seize(5.0);
        y.tick(10.0); // 5 - 100 → 0
        assert_eq!(y.claim, 0.0);
    }

    #[test]
    fn tick_fires_just_lost_on_erode_to_zero() {
        let mut y = Yours::new(100.0, 10.0);
        y.seize(5.0);
        y.tick(1.0);
        assert!(y.just_lost);
        assert!(y.is_lost());
    }

    #[test]
    fn tick_no_lost_when_already_unclaimed() {
        let mut y = Yours::new(100.0, 10.0);
        y.tick(1.0); // already 0
        assert!(!y.just_lost);
    }

    #[test]
    fn tick_clears_just_claimed() {
        let mut y = y();
        y.seize(100.0);
        y.tick(0.016);
        assert!(!y.just_claimed);
    }

    #[test]
    fn tick_clears_just_lost() {
        let mut y = y();
        y.seize(30.0);
        y.contest(30.0); // just_lost fires
        y.tick(0.016); // cleared
        assert!(!y.just_lost);
    }

    #[test]
    fn tick_no_erosion_when_rate_zero() {
        let mut y = y();
        y.seize(50.0);
        y.tick(100.0); // no erosion
        assert!((y.claim - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_erosion_when_disabled() {
        let mut y = Yours::new(100.0, 10.0);
        y.seize(50.0);
        y.enabled = false;
        y.tick(1.0);
        assert!((y.claim - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut y = Yours::new(100.0, 10.0);
        y.seize(80.0);
        y.tick(0.5); // 80 - 5 = 75
        assert!((y.claim - 75.0).abs() < 1e-3);
    }

    // --- is_claimed / is_lost ---

    #[test]
    fn is_claimed_false_below_max() {
        let mut y = y();
        y.seize(50.0);
        assert!(!y.is_claimed());
    }

    #[test]
    fn is_claimed_false_when_disabled() {
        let mut y = y();
        y.seize(100.0);
        y.enabled = false;
        assert!(!y.is_claimed());
    }

    #[test]
    fn is_lost_true_at_zero() {
        assert!(y().is_lost());
    }

    #[test]
    fn is_lost_not_gated_by_enabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_lost());
    }

    // --- fractions / effective ---

    #[test]
    fn claim_fraction_zero_when_unclaimed() {
        assert_eq!(y().claim_fraction(), 0.0);
    }

    #[test]
    fn claim_fraction_half_at_midpoint() {
        let mut y = y();
        y.seize(50.0);
        assert!((y.claim_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn claim_fraction_one_at_max() {
        let mut y = y();
        y.seize(100.0);
        assert!((y.claim_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_control_zero_when_unclaimed() {
        assert_eq!(y().effective_control(100.0), 0.0);
    }

    #[test]
    fn effective_control_scales_with_fraction() {
        let mut y = y();
        y.seize(75.0);
        assert!((y.effective_control(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_control_zero_when_disabled() {
        let mut y = y();
        y.seize(50.0);
        y.enabled = false;
        assert_eq!(y.effective_control(100.0), 0.0);
    }
}
