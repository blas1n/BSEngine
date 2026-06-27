use bevy_ecs::prelude::Component;

/// Élan-fighting-spirit tracker. `elan` builds via `flare(amount)` and
/// surges passively at `surge_rate` per second in `tick(dt)` or
/// falters immediately via `falter(amount)`.
///
/// Models light-infantry élan meters, martial-bravado fill levels,
/// skirmisher-courage accumulators, flamboyant-duelist style bars,
/// parade-ground-drill progress gauges, cavalry-charge spirit indicators,
/// berserker-display build-up trackers, or any mechanic where a
/// warrior's theatrical audacity amplifies combat effectiveness.
///
/// `flare(amount)` adds elan; fires `just_dashing` when first reaching
/// `max_elan`. No-op when disabled.
///
/// `falter(amount)` reduces elan immediately; fires `just_broken` when
/// reaching 0. No-op when disabled or already broken.
///
/// `tick(dt)` clears both flags, then increases elan by
/// `surge_rate * dt` (capped at `max_elan`). Fires `just_dashing`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_dashing()` returns `elan >= max_elan && enabled`.
///
/// `is_broken()` returns `elan == 0.0` (not gated by `enabled`).
///
/// `elan_fraction()` returns `(elan / max_elan).clamp(0, 1)`.
///
/// `effective_panache(scale)` returns `scale * elan_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 5.0)` — surges at 5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zouave {
    pub elan: f32,
    pub max_elan: f32,
    pub surge_rate: f32,
    pub just_dashing: bool,
    pub just_broken: bool,
    pub enabled: bool,
}

impl Zouave {
    pub fn new(max_elan: f32, surge_rate: f32) -> Self {
        Self {
            elan: 0.0,
            max_elan: max_elan.max(0.1),
            surge_rate: surge_rate.max(0.0),
            just_dashing: false,
            just_broken: false,
            enabled: true,
        }
    }

    /// Add elan; fires `just_dashing` when first reaching max.
    /// No-op when disabled.
    pub fn flare(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.elan < self.max_elan;
        self.elan = (self.elan + amount).min(self.max_elan);
        if was_below && self.elan >= self.max_elan {
            self.just_dashing = true;
        }
    }

    /// Reduce elan; fires `just_broken` when reaching 0.
    /// No-op when disabled or already broken.
    pub fn falter(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.elan <= 0.0 {
            return;
        }
        self.elan = (self.elan - amount).max(0.0);
        if self.elan <= 0.0 {
            self.just_broken = true;
        }
    }

    /// Clear flags, then increase elan by `surge_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_dashing = false;
        self.just_broken = false;
        if self.enabled && self.surge_rate > 0.0 && self.elan < self.max_elan {
            let was_below = self.elan < self.max_elan;
            self.elan = (self.elan + self.surge_rate * dt).min(self.max_elan);
            if was_below && self.elan >= self.max_elan {
                self.just_dashing = true;
            }
        }
    }

    /// `true` when elan is at maximum and component is enabled.
    pub fn is_dashing(&self) -> bool {
        self.elan >= self.max_elan && self.enabled
    }

    /// `true` when elan is 0 (not gated by `enabled`).
    pub fn is_broken(&self) -> bool {
        self.elan == 0.0
    }

    /// Fraction of maximum elan [0.0, 1.0].
    pub fn elan_fraction(&self) -> f32 {
        (self.elan / self.max_elan).clamp(0.0, 1.0)
    }

    /// Returns `scale * elan_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_panache(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.elan_fraction()
    }
}

impl Default for Zouave {
    fn default() -> Self {
        Self::new(100.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zouave {
        Zouave::new(100.0, 5.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_broken() {
        let z = z();
        assert_eq!(z.elan, 0.0);
        assert!(z.is_broken());
        assert!(!z.is_dashing());
    }

    #[test]
    fn new_clamps_max_elan() {
        let z = Zouave::new(-5.0, 5.0);
        assert!((z.max_elan - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_surge_rate() {
        let z = Zouave::new(100.0, -3.0);
        assert_eq!(z.surge_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zouave::default();
        assert!((z.max_elan - 100.0).abs() < 1e-5);
        assert!((z.surge_rate - 5.0).abs() < 1e-5);
    }

    // --- flare ---

    #[test]
    fn flare_adds_elan() {
        let mut z = z();
        z.flare(40.0);
        assert!((z.elan - 40.0).abs() < 1e-3);
    }

    #[test]
    fn flare_clamps_at_max() {
        let mut z = z();
        z.flare(200.0);
        assert!((z.elan - 100.0).abs() < 1e-3);
    }

    #[test]
    fn flare_fires_just_dashing_at_max() {
        let mut z = z();
        z.flare(100.0);
        assert!(z.just_dashing);
        assert!(z.is_dashing());
    }

    #[test]
    fn flare_no_just_dashing_when_already_at_max() {
        let mut z = z();
        z.elan = 100.0;
        z.flare(10.0);
        assert!(!z.just_dashing);
    }

    #[test]
    fn flare_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.flare(50.0);
        assert_eq!(z.elan, 0.0);
    }

    #[test]
    fn flare_no_op_when_amount_zero() {
        let mut z = z();
        z.flare(0.0);
        assert_eq!(z.elan, 0.0);
    }

    // --- falter ---

    #[test]
    fn falter_reduces_elan() {
        let mut z = z();
        z.elan = 60.0;
        z.falter(20.0);
        assert!((z.elan - 40.0).abs() < 1e-3);
    }

    #[test]
    fn falter_clamps_at_zero() {
        let mut z = z();
        z.elan = 30.0;
        z.falter(200.0);
        assert_eq!(z.elan, 0.0);
    }

    #[test]
    fn falter_fires_just_broken_at_zero() {
        let mut z = z();
        z.elan = 30.0;
        z.falter(30.0);
        assert!(z.just_broken);
    }

    #[test]
    fn falter_no_op_when_already_broken() {
        let mut z = z();
        z.falter(10.0);
        assert!(!z.just_broken);
    }

    #[test]
    fn falter_no_op_when_disabled() {
        let mut z = z();
        z.elan = 50.0;
        z.enabled = false;
        z.falter(50.0);
        assert!((z.elan - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_surges_elan() {
        let mut z = z(); // rate=5
        z.tick(1.0); // 0 + 5 = 5
        assert!((z.elan - 5.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_dashing_on_surge_to_max() {
        let mut z = Zouave::new(100.0, 200.0);
        z.elan = 95.0;
        z.tick(1.0);
        assert!(z.just_dashing);
        assert!(z.is_dashing());
    }

    #[test]
    fn tick_no_surge_when_already_dashing() {
        let mut z = z();
        z.elan = 100.0;
        z.tick(1.0);
        assert!(!z.just_dashing);
    }

    #[test]
    fn tick_no_surge_when_rate_zero() {
        let mut z = Zouave::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.elan, 0.0);
    }

    #[test]
    fn tick_no_surge_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.elan, 0.0);
    }

    #[test]
    fn tick_clears_just_dashing() {
        let mut z = Zouave::new(100.0, 200.0);
        z.elan = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_dashing);
    }

    #[test]
    fn tick_clears_just_broken() {
        let mut z = z();
        z.elan = 10.0;
        z.falter(10.0);
        z.tick(0.016);
        assert!(!z.just_broken);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=5
        z.tick(3.0); // 5*3 = 15
        assert!((z.elan - 15.0).abs() < 1e-3);
    }

    // --- is_dashing / is_broken ---

    #[test]
    fn is_dashing_false_when_disabled() {
        let mut z = z();
        z.elan = 100.0;
        z.enabled = false;
        assert!(!z.is_dashing());
    }

    #[test]
    fn is_broken_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_broken());
    }

    // --- elan_fraction / effective_panache ---

    #[test]
    fn elan_fraction_zero_when_broken() {
        assert_eq!(z().elan_fraction(), 0.0);
    }

    #[test]
    fn elan_fraction_half_at_midpoint() {
        let mut z = z();
        z.elan = 50.0;
        assert!((z.elan_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_panache_zero_when_broken() {
        assert_eq!(z().effective_panache(100.0), 0.0);
    }

    #[test]
    fn effective_panache_scales_with_elan() {
        let mut z = z();
        z.elan = 80.0;
        assert!((z.effective_panache(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_panache_zero_when_disabled() {
        let mut z = z();
        z.elan = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_panache(100.0), 0.0);
    }
}
