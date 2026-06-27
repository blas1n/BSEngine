use bevy_ecs::prelude::Component;

/// Musk-defense tracker. `musk` builds via `spray(amount)` and
/// intensifies passively at `musk_rate` per second in `tick(dt)` or
/// dissipates immediately via `dissipate(amount)`.
///
/// Models striped-polecat defense-spray meters, chemical-deterrent
/// fill levels, scent-marking accumulation trackers, skunk-like
/// noxious-cloud build-up indicators, mustelid-threat escalation
/// gauges, chemical-weapon charge bars, repellent-cloud saturation
/// meters, predator-deterrent intensity trackers, or any mechanic
/// where a small but fearless animal charges a musk gland to full
/// potency, saturating the air with a stench that drives even lions
/// to retreat before the zorilla's tiny striped form.
///
/// `spray(amount)` adds musk; fires `just_reeking` when first
/// reaching `max_musk`. No-op when disabled.
///
/// `dissipate(amount)` reduces musk immediately; fires `just_fresh`
/// when reaching 0. No-op when disabled or already fresh.
///
/// `tick(dt)` clears both flags, then increases musk by
/// `musk_rate * dt` (capped at `max_musk`). Fires `just_reeking`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_reeking()` returns `musk >= max_musk && enabled`.
///
/// `is_fresh()` returns `musk == 0.0` (not gated by `enabled`).
///
/// `musk_fraction()` returns `(musk / max_musk).clamp(0, 1)`.
///
/// `effective_stench(scale)` returns `scale * musk_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 5.0)` — charges musk at 5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zorilla {
    pub musk: f32,
    pub max_musk: f32,
    pub musk_rate: f32,
    pub just_reeking: bool,
    pub just_fresh: bool,
    pub enabled: bool,
}

impl Zorilla {
    pub fn new(max_musk: f32, musk_rate: f32) -> Self {
        Self {
            musk: 0.0,
            max_musk: max_musk.max(0.1),
            musk_rate: musk_rate.max(0.0),
            just_reeking: false,
            just_fresh: false,
            enabled: true,
        }
    }

    /// Add musk; fires `just_reeking` when first reaching max.
    /// No-op when disabled.
    pub fn spray(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.musk < self.max_musk;
        self.musk = (self.musk + amount).min(self.max_musk);
        if was_below && self.musk >= self.max_musk {
            self.just_reeking = true;
        }
    }

    /// Reduce musk; fires `just_fresh` when reaching 0.
    /// No-op when disabled or already fresh.
    pub fn dissipate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.musk <= 0.0 {
            return;
        }
        self.musk = (self.musk - amount).max(0.0);
        if self.musk <= 0.0 {
            self.just_fresh = true;
        }
    }

    /// Clear flags, then increase musk by `musk_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_reeking = false;
        self.just_fresh = false;
        if self.enabled && self.musk_rate > 0.0 && self.musk < self.max_musk {
            let was_below = self.musk < self.max_musk;
            self.musk = (self.musk + self.musk_rate * dt).min(self.max_musk);
            if was_below && self.musk >= self.max_musk {
                self.just_reeking = true;
            }
        }
    }

    /// `true` when musk is at maximum and component is enabled.
    pub fn is_reeking(&self) -> bool {
        self.musk >= self.max_musk && self.enabled
    }

    /// `true` when musk is 0 (not gated by `enabled`).
    pub fn is_fresh(&self) -> bool {
        self.musk == 0.0
    }

    /// Fraction of maximum musk [0.0, 1.0].
    pub fn musk_fraction(&self) -> f32 {
        (self.musk / self.max_musk).clamp(0.0, 1.0)
    }

    /// Returns `scale * musk_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_stench(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.musk_fraction()
    }
}

impl Default for Zorilla {
    fn default() -> Self {
        Self::new(100.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zorilla {
        Zorilla::new(100.0, 5.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_fresh() {
        let z = z();
        assert_eq!(z.musk, 0.0);
        assert!(z.is_fresh());
        assert!(!z.is_reeking());
    }

    #[test]
    fn new_clamps_max_musk() {
        let z = Zorilla::new(-5.0, 5.0);
        assert!((z.max_musk - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_musk_rate() {
        let z = Zorilla::new(100.0, -3.0);
        assert_eq!(z.musk_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zorilla::default();
        assert!((z.max_musk - 100.0).abs() < 1e-5);
        assert!((z.musk_rate - 5.0).abs() < 1e-5);
    }

    // --- spray ---

    #[test]
    fn spray_adds_musk() {
        let mut z = z();
        z.spray(40.0);
        assert!((z.musk - 40.0).abs() < 1e-3);
    }

    #[test]
    fn spray_clamps_at_max() {
        let mut z = z();
        z.spray(200.0);
        assert!((z.musk - 100.0).abs() < 1e-3);
    }

    #[test]
    fn spray_fires_just_reeking_at_max() {
        let mut z = z();
        z.spray(100.0);
        assert!(z.just_reeking);
        assert!(z.is_reeking());
    }

    #[test]
    fn spray_no_just_reeking_when_already_at_max() {
        let mut z = z();
        z.musk = 100.0;
        z.spray(10.0);
        assert!(!z.just_reeking);
    }

    #[test]
    fn spray_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.spray(50.0);
        assert_eq!(z.musk, 0.0);
    }

    #[test]
    fn spray_no_op_when_amount_zero() {
        let mut z = z();
        z.spray(0.0);
        assert_eq!(z.musk, 0.0);
    }

    // --- dissipate ---

    #[test]
    fn dissipate_reduces_musk() {
        let mut z = z();
        z.musk = 60.0;
        z.dissipate(20.0);
        assert!((z.musk - 40.0).abs() < 1e-3);
    }

    #[test]
    fn dissipate_clamps_at_zero() {
        let mut z = z();
        z.musk = 30.0;
        z.dissipate(200.0);
        assert_eq!(z.musk, 0.0);
    }

    #[test]
    fn dissipate_fires_just_fresh_at_zero() {
        let mut z = z();
        z.musk = 30.0;
        z.dissipate(30.0);
        assert!(z.just_fresh);
    }

    #[test]
    fn dissipate_no_op_when_already_fresh() {
        let mut z = z();
        z.dissipate(10.0);
        assert!(!z.just_fresh);
    }

    #[test]
    fn dissipate_no_op_when_disabled() {
        let mut z = z();
        z.musk = 50.0;
        z.enabled = false;
        z.dissipate(50.0);
        assert!((z.musk - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_charges_musk() {
        let mut z = z(); // rate=5
        z.tick(2.0); // 0 + 5*2 = 10
        assert!((z.musk - 10.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_reeking_on_charge_to_max() {
        let mut z = Zorilla::new(100.0, 200.0);
        z.musk = 95.0;
        z.tick(1.0);
        assert!(z.just_reeking);
        assert!(z.is_reeking());
    }

    #[test]
    fn tick_no_charge_when_already_reeking() {
        let mut z = z();
        z.musk = 100.0;
        z.tick(1.0);
        assert!(!z.just_reeking);
    }

    #[test]
    fn tick_no_charge_when_rate_zero() {
        let mut z = Zorilla::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.musk, 0.0);
    }

    #[test]
    fn tick_no_charge_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.musk, 0.0);
    }

    #[test]
    fn tick_clears_just_reeking() {
        let mut z = Zorilla::new(100.0, 200.0);
        z.musk = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_reeking);
    }

    #[test]
    fn tick_clears_just_fresh() {
        let mut z = z();
        z.musk = 10.0;
        z.dissipate(10.0);
        z.tick(0.016);
        assert!(!z.just_fresh);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=5
        z.tick(4.0); // 5*4 = 20
        assert!((z.musk - 20.0).abs() < 1e-3);
    }

    // --- is_reeking / is_fresh ---

    #[test]
    fn is_reeking_false_when_disabled() {
        let mut z = z();
        z.musk = 100.0;
        z.enabled = false;
        assert!(!z.is_reeking());
    }

    #[test]
    fn is_fresh_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_fresh());
    }

    // --- musk_fraction / effective_stench ---

    #[test]
    fn musk_fraction_zero_when_fresh() {
        assert_eq!(z().musk_fraction(), 0.0);
    }

    #[test]
    fn musk_fraction_half_at_midpoint() {
        let mut z = z();
        z.musk = 50.0;
        assert!((z.musk_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_stench_zero_when_fresh() {
        assert_eq!(z().effective_stench(100.0), 0.0);
    }

    #[test]
    fn effective_stench_scales_with_musk() {
        let mut z = z();
        z.musk = 65.0;
        assert!((z.effective_stench(100.0) - 65.0).abs() < 1e-3);
    }

    #[test]
    fn effective_stench_zero_when_disabled() {
        let mut z = z();
        z.musk = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_stench(100.0), 0.0);
    }
}
