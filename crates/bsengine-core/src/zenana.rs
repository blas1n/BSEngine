use bevy_ecs::prelude::Component;

/// Sanctuary-solace tracker. `solace` builds via `retreat(amount)` and
/// deepens passively at `shelter_rate` per second in `tick(dt)` or
/// dissipates immediately via `disturb(amount)`.
///
/// Models private-apartment sanctuary meters, women's-quarter refuge
/// fill levels, inner-sanctum solace accumulators, household-retreat
/// depth gauges, haven-of-peace escalation trackers, cloistered-space
/// calm indicators, personal-refuge accumulation bars, sheltered-
/// courtyard tranquillity progress trackers, or any mechanic where
/// withdrawing into a protected inner space progressively deepens a
/// sense of inviolable calm that can be shattered by outside intrusion.
///
/// `retreat(amount)` adds solace; fires `just_secluded` when first
/// reaching `max_solace`. No-op when disabled.
///
/// `disturb(amount)` reduces solace immediately; fires `just_disturbed`
/// when reaching 0. No-op when disabled or already disturbed.
///
/// `tick(dt)` clears both flags, then increases solace by
/// `shelter_rate * dt` (capped at `max_solace`). Fires `just_secluded`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_secluded()` returns `solace >= max_solace && enabled`.
///
/// `is_disturbed()` returns `solace == 0.0` (not gated by `enabled`).
///
/// `solace_fraction()` returns `(solace / max_solace).clamp(0, 1)`.
///
/// `effective_refuge(scale)` returns `scale * solace_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — shelters at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zenana {
    pub solace: f32,
    pub max_solace: f32,
    pub shelter_rate: f32,
    pub just_secluded: bool,
    pub just_disturbed: bool,
    pub enabled: bool,
}

impl Zenana {
    pub fn new(max_solace: f32, shelter_rate: f32) -> Self {
        Self {
            solace: 0.0,
            max_solace: max_solace.max(0.1),
            shelter_rate: shelter_rate.max(0.0),
            just_secluded: false,
            just_disturbed: false,
            enabled: true,
        }
    }

    /// Add solace; fires `just_secluded` when first reaching max.
    /// No-op when disabled.
    pub fn retreat(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.solace < self.max_solace;
        self.solace = (self.solace + amount).min(self.max_solace);
        if was_below && self.solace >= self.max_solace {
            self.just_secluded = true;
        }
    }

    /// Reduce solace; fires `just_disturbed` when reaching 0.
    /// No-op when disabled or already disturbed.
    pub fn disturb(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.solace <= 0.0 {
            return;
        }
        self.solace = (self.solace - amount).max(0.0);
        if self.solace <= 0.0 {
            self.just_disturbed = true;
        }
    }

    /// Clear flags, then increase solace by `shelter_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_secluded = false;
        self.just_disturbed = false;
        if self.enabled && self.shelter_rate > 0.0 && self.solace < self.max_solace {
            let was_below = self.solace < self.max_solace;
            self.solace = (self.solace + self.shelter_rate * dt).min(self.max_solace);
            if was_below && self.solace >= self.max_solace {
                self.just_secluded = true;
            }
        }
    }

    /// `true` when solace is at maximum and component is enabled.
    pub fn is_secluded(&self) -> bool {
        self.solace >= self.max_solace && self.enabled
    }

    /// `true` when solace is 0 (not gated by `enabled`).
    pub fn is_disturbed(&self) -> bool {
        self.solace == 0.0
    }

    /// Fraction of maximum solace [0.0, 1.0].
    pub fn solace_fraction(&self) -> f32 {
        (self.solace / self.max_solace).clamp(0.0, 1.0)
    }

    /// Returns `scale * solace_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_refuge(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.solace_fraction()
    }
}

impl Default for Zenana {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zenana {
        Zenana::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_disturbed() {
        let z = z();
        assert_eq!(z.solace, 0.0);
        assert!(z.is_disturbed());
        assert!(!z.is_secluded());
    }

    #[test]
    fn new_clamps_max_solace() {
        let z = Zenana::new(-5.0, 1.5);
        assert!((z.max_solace - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_shelter_rate() {
        let z = Zenana::new(100.0, -3.0);
        assert_eq!(z.shelter_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zenana::default();
        assert!((z.max_solace - 100.0).abs() < 1e-5);
        assert!((z.shelter_rate - 1.5).abs() < 1e-5);
    }

    // --- retreat ---

    #[test]
    fn retreat_adds_solace() {
        let mut z = z();
        z.retreat(40.0);
        assert!((z.solace - 40.0).abs() < 1e-3);
    }

    #[test]
    fn retreat_clamps_at_max() {
        let mut z = z();
        z.retreat(200.0);
        assert!((z.solace - 100.0).abs() < 1e-3);
    }

    #[test]
    fn retreat_fires_just_secluded_at_max() {
        let mut z = z();
        z.retreat(100.0);
        assert!(z.just_secluded);
        assert!(z.is_secluded());
    }

    #[test]
    fn retreat_no_just_secluded_when_already_at_max() {
        let mut z = z();
        z.solace = 100.0;
        z.retreat(10.0);
        assert!(!z.just_secluded);
    }

    #[test]
    fn retreat_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.retreat(50.0);
        assert_eq!(z.solace, 0.0);
    }

    #[test]
    fn retreat_no_op_when_amount_zero() {
        let mut z = z();
        z.retreat(0.0);
        assert_eq!(z.solace, 0.0);
    }

    // --- disturb ---

    #[test]
    fn disturb_reduces_solace() {
        let mut z = z();
        z.solace = 60.0;
        z.disturb(20.0);
        assert!((z.solace - 40.0).abs() < 1e-3);
    }

    #[test]
    fn disturb_clamps_at_zero() {
        let mut z = z();
        z.solace = 30.0;
        z.disturb(200.0);
        assert_eq!(z.solace, 0.0);
    }

    #[test]
    fn disturb_fires_just_disturbed_at_zero() {
        let mut z = z();
        z.solace = 30.0;
        z.disturb(30.0);
        assert!(z.just_disturbed);
    }

    #[test]
    fn disturb_no_op_when_already_disturbed() {
        let mut z = z();
        z.disturb(10.0);
        assert!(!z.just_disturbed);
    }

    #[test]
    fn disturb_no_op_when_disabled() {
        let mut z = z();
        z.solace = 50.0;
        z.enabled = false;
        z.disturb(50.0);
        assert!((z.solace - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_shelters_solace() {
        let mut z = z(); // rate=1.5
        z.tick(2.0); // 0 + 1.5*2 = 3
        assert!((z.solace - 3.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_secluded_on_shelter_to_max() {
        let mut z = Zenana::new(100.0, 200.0);
        z.solace = 95.0;
        z.tick(1.0);
        assert!(z.just_secluded);
        assert!(z.is_secluded());
    }

    #[test]
    fn tick_no_shelter_when_already_secluded() {
        let mut z = z();
        z.solace = 100.0;
        z.tick(1.0);
        assert!(!z.just_secluded);
    }

    #[test]
    fn tick_no_shelter_when_rate_zero() {
        let mut z = Zenana::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.solace, 0.0);
    }

    #[test]
    fn tick_no_shelter_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.solace, 0.0);
    }

    #[test]
    fn tick_clears_just_secluded() {
        let mut z = Zenana::new(100.0, 200.0);
        z.solace = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_secluded);
    }

    #[test]
    fn tick_clears_just_disturbed() {
        let mut z = z();
        z.solace = 10.0;
        z.disturb(10.0);
        z.tick(0.016);
        assert!(!z.just_disturbed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 1.5*4 = 6
        assert!((z.solace - 6.0).abs() < 1e-3);
    }

    // --- is_secluded / is_disturbed ---

    #[test]
    fn is_secluded_false_when_disabled() {
        let mut z = z();
        z.solace = 100.0;
        z.enabled = false;
        assert!(!z.is_secluded());
    }

    #[test]
    fn is_disturbed_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_disturbed());
    }

    // --- solace_fraction / effective_refuge ---

    #[test]
    fn solace_fraction_zero_when_disturbed() {
        assert_eq!(z().solace_fraction(), 0.0);
    }

    #[test]
    fn solace_fraction_half_at_midpoint() {
        let mut z = z();
        z.solace = 50.0;
        assert!((z.solace_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_refuge_zero_when_disturbed() {
        assert_eq!(z().effective_refuge(100.0), 0.0);
    }

    #[test]
    fn effective_refuge_scales_with_solace() {
        let mut z = z();
        z.solace = 60.0;
        assert!((z.effective_refuge(100.0) - 60.0).abs() < 1e-3);
    }

    #[test]
    fn effective_refuge_zero_when_disabled() {
        let mut z = z();
        z.solace = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_refuge(100.0), 0.0);
    }
}
