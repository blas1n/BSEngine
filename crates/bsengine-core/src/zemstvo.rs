use bevy_ecs::prelude::Component;

/// Council-authority tracker. `authority` builds via `convene(amount)`
/// and grows passively at `mandate_rate` per second in `tick(dt)` or
/// is dissolved immediately via `dissolve(amount)`.
///
/// Models Russian-district-assembly mandate meters, local-council
/// authority fill levels, civic-governance legitimacy accumulators,
/// provincial-assembly power gauges, elected-body influence trackers,
/// grassroots-democracy authority indicators, village-council standing
/// progress bars, representative-assembly mandate trackers, or any
/// mechanic where a regional council steadily builds its authority
/// through public mandates until it speaks with the full voice of its
/// district — only to be dissolved by a stroke of central decree.
///
/// `convene(amount)` adds authority; fires `just_empowered` when first
/// reaching `max_authority`. No-op when disabled.
///
/// `dissolve(amount)` reduces authority immediately; fires `just_abolished`
/// when reaching 0. No-op when disabled or already abolished.
///
/// `tick(dt)` clears both flags, then increases authority by
/// `mandate_rate * dt` (capped at `max_authority`). Fires `just_empowered`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_empowered()` returns `authority >= max_authority && enabled`.
///
/// `is_abolished()` returns `authority == 0.0` (not gated by `enabled`).
///
/// `authority_fraction()` returns `(authority / max_authority).clamp(0, 1)`.
///
/// `effective_mandate(scale)` returns `scale * authority_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — accumulates mandate at 2 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zemstvo {
    pub authority: f32,
    pub max_authority: f32,
    pub mandate_rate: f32,
    pub just_empowered: bool,
    pub just_abolished: bool,
    pub enabled: bool,
}

impl Zemstvo {
    pub fn new(max_authority: f32, mandate_rate: f32) -> Self {
        Self {
            authority: 0.0,
            max_authority: max_authority.max(0.1),
            mandate_rate: mandate_rate.max(0.0),
            just_empowered: false,
            just_abolished: false,
            enabled: true,
        }
    }

    /// Add authority; fires `just_empowered` when first reaching max.
    /// No-op when disabled.
    pub fn convene(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.authority < self.max_authority;
        self.authority = (self.authority + amount).min(self.max_authority);
        if was_below && self.authority >= self.max_authority {
            self.just_empowered = true;
        }
    }

    /// Reduce authority; fires `just_abolished` when reaching 0.
    /// No-op when disabled or already abolished.
    pub fn dissolve(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.authority <= 0.0 {
            return;
        }
        self.authority = (self.authority - amount).max(0.0);
        if self.authority <= 0.0 {
            self.just_abolished = true;
        }
    }

    /// Clear flags, then increase authority by `mandate_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_empowered = false;
        self.just_abolished = false;
        if self.enabled && self.mandate_rate > 0.0 && self.authority < self.max_authority {
            let was_below = self.authority < self.max_authority;
            self.authority = (self.authority + self.mandate_rate * dt).min(self.max_authority);
            if was_below && self.authority >= self.max_authority {
                self.just_empowered = true;
            }
        }
    }

    /// `true` when authority is at maximum and component is enabled.
    pub fn is_empowered(&self) -> bool {
        self.authority >= self.max_authority && self.enabled
    }

    /// `true` when authority is 0 (not gated by `enabled`).
    pub fn is_abolished(&self) -> bool {
        self.authority == 0.0
    }

    /// Fraction of maximum authority [0.0, 1.0].
    pub fn authority_fraction(&self) -> f32 {
        (self.authority / self.max_authority).clamp(0.0, 1.0)
    }

    /// Returns `scale * authority_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_mandate(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.authority_fraction()
    }
}

impl Default for Zemstvo {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zemstvo {
        Zemstvo::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_abolished() {
        let z = z();
        assert_eq!(z.authority, 0.0);
        assert!(z.is_abolished());
        assert!(!z.is_empowered());
    }

    #[test]
    fn new_clamps_max_authority() {
        let z = Zemstvo::new(-5.0, 2.0);
        assert!((z.max_authority - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_mandate_rate() {
        let z = Zemstvo::new(100.0, -3.0);
        assert_eq!(z.mandate_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zemstvo::default();
        assert!((z.max_authority - 100.0).abs() < 1e-5);
        assert!((z.mandate_rate - 2.0).abs() < 1e-5);
    }

    // --- convene ---

    #[test]
    fn convene_adds_authority() {
        let mut z = z();
        z.convene(40.0);
        assert!((z.authority - 40.0).abs() < 1e-3);
    }

    #[test]
    fn convene_clamps_at_max() {
        let mut z = z();
        z.convene(200.0);
        assert!((z.authority - 100.0).abs() < 1e-3);
    }

    #[test]
    fn convene_fires_just_empowered_at_max() {
        let mut z = z();
        z.convene(100.0);
        assert!(z.just_empowered);
        assert!(z.is_empowered());
    }

    #[test]
    fn convene_no_just_empowered_when_already_at_max() {
        let mut z = z();
        z.authority = 100.0;
        z.convene(10.0);
        assert!(!z.just_empowered);
    }

    #[test]
    fn convene_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.convene(50.0);
        assert_eq!(z.authority, 0.0);
    }

    #[test]
    fn convene_no_op_when_amount_zero() {
        let mut z = z();
        z.convene(0.0);
        assert_eq!(z.authority, 0.0);
    }

    // --- dissolve ---

    #[test]
    fn dissolve_reduces_authority() {
        let mut z = z();
        z.authority = 60.0;
        z.dissolve(20.0);
        assert!((z.authority - 40.0).abs() < 1e-3);
    }

    #[test]
    fn dissolve_clamps_at_zero() {
        let mut z = z();
        z.authority = 30.0;
        z.dissolve(200.0);
        assert_eq!(z.authority, 0.0);
    }

    #[test]
    fn dissolve_fires_just_abolished_at_zero() {
        let mut z = z();
        z.authority = 30.0;
        z.dissolve(30.0);
        assert!(z.just_abolished);
    }

    #[test]
    fn dissolve_no_op_when_already_abolished() {
        let mut z = z();
        z.dissolve(10.0);
        assert!(!z.just_abolished);
    }

    #[test]
    fn dissolve_no_op_when_disabled() {
        let mut z = z();
        z.authority = 50.0;
        z.enabled = false;
        z.dissolve(50.0);
        assert!((z.authority - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_grows_authority() {
        let mut z = z(); // rate=2
        z.tick(3.0); // 0 + 2*3 = 6
        assert!((z.authority - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_empowered_on_mandate_to_max() {
        let mut z = Zemstvo::new(100.0, 200.0);
        z.authority = 95.0;
        z.tick(1.0);
        assert!(z.just_empowered);
        assert!(z.is_empowered());
    }

    #[test]
    fn tick_no_growth_when_already_empowered() {
        let mut z = z();
        z.authority = 100.0;
        z.tick(1.0);
        assert!(!z.just_empowered);
    }

    #[test]
    fn tick_no_growth_when_rate_zero() {
        let mut z = Zemstvo::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.authority, 0.0);
    }

    #[test]
    fn tick_no_growth_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.authority, 0.0);
    }

    #[test]
    fn tick_clears_just_empowered() {
        let mut z = Zemstvo::new(100.0, 200.0);
        z.authority = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_empowered);
    }

    #[test]
    fn tick_clears_just_abolished() {
        let mut z = z();
        z.authority = 10.0;
        z.dissolve(10.0);
        z.tick(0.016);
        assert!(!z.just_abolished);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2
        z.tick(5.0); // 2*5 = 10
        assert!((z.authority - 10.0).abs() < 1e-3);
    }

    // --- is_empowered / is_abolished ---

    #[test]
    fn is_empowered_false_when_disabled() {
        let mut z = z();
        z.authority = 100.0;
        z.enabled = false;
        assert!(!z.is_empowered());
    }

    #[test]
    fn is_abolished_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_abolished());
    }

    // --- authority_fraction / effective_mandate ---

    #[test]
    fn authority_fraction_zero_when_abolished() {
        assert_eq!(z().authority_fraction(), 0.0);
    }

    #[test]
    fn authority_fraction_half_at_midpoint() {
        let mut z = z();
        z.authority = 50.0;
        assert!((z.authority_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_mandate_zero_when_abolished() {
        assert_eq!(z().effective_mandate(100.0), 0.0);
    }

    #[test]
    fn effective_mandate_scales_with_authority() {
        let mut z = z();
        z.authority = 70.0;
        assert!((z.effective_mandate(100.0) - 70.0).abs() < 1e-3);
    }

    #[test]
    fn effective_mandate_zero_when_disabled() {
        let mut z = z();
        z.authority = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_mandate(100.0), 0.0);
    }
}
