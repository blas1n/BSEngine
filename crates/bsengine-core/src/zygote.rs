use bevy_ecs::prelude::Component;

/// Cell-development potential tracker. `potency` builds via
/// `fuse(amount)` and increases passively at `division_rate` per
/// second in `tick(dt)` or is arrested immediately via
/// `arrest(amount)`.
///
/// Models embryonic-development progress bars, cell-division
/// accumulation meters, genetic-expression saturation gauges,
/// morphogenesis-potential fill levels, blastocyst-growth intensity
/// trackers, stem-cell potency build-up bars, gastrulation-progress
/// indicators, mitotic-activity fill levels, totipotency-saturation
/// trackers, or any mechanic where a single fertilized cell quietly
/// divides itself through every stage of differentiation until it
/// has assembled an entire organism — one mitosis at a time — only
/// for developmental arrest to halt the whole cascade and leave
/// potency suspended, waiting for the right signal to resume.
///
/// `fuse(amount)` adds potency; fires `just_developed` when first
/// reaching `max_potency`. No-op when disabled.
///
/// `arrest(amount)` reduces potency immediately; fires
/// `just_arrested` when reaching 0. No-op when disabled or already
/// arrested.
///
/// `tick(dt)` clears both flags, then increases potency by
/// `division_rate * dt` (capped at `max_potency`). Fires
/// `just_developed` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_developed()` returns `potency >= max_potency && enabled`.
///
/// `is_arrested()` returns `potency == 0.0` (not gated by `enabled`).
///
/// `potency_fraction()` returns `(potency / max_potency).clamp(0, 1)`.
///
/// `effective_vitality(scale)` returns `scale * potency_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — divides at 2 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zygote {
    pub potency: f32,
    pub max_potency: f32,
    pub division_rate: f32,
    pub just_developed: bool,
    pub just_arrested: bool,
    pub enabled: bool,
}

impl Zygote {
    pub fn new(max_potency: f32, division_rate: f32) -> Self {
        Self {
            potency: 0.0,
            max_potency: max_potency.max(0.1),
            division_rate: division_rate.max(0.0),
            just_developed: false,
            just_arrested: false,
            enabled: true,
        }
    }

    /// Add potency; fires `just_developed` when first reaching max.
    /// No-op when disabled.
    pub fn fuse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.potency < self.max_potency;
        self.potency = (self.potency + amount).min(self.max_potency);
        if was_below && self.potency >= self.max_potency {
            self.just_developed = true;
        }
    }

    /// Reduce potency; fires `just_arrested` when reaching 0.
    /// No-op when disabled or already arrested.
    pub fn arrest(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.potency <= 0.0 {
            return;
        }
        self.potency = (self.potency - amount).max(0.0);
        if self.potency <= 0.0 {
            self.just_arrested = true;
        }
    }

    /// Clear flags, then increase potency by `division_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_developed = false;
        self.just_arrested = false;
        if self.enabled && self.division_rate > 0.0 && self.potency < self.max_potency {
            let was_below = self.potency < self.max_potency;
            self.potency = (self.potency + self.division_rate * dt).min(self.max_potency);
            if was_below && self.potency >= self.max_potency {
                self.just_developed = true;
            }
        }
    }

    /// `true` when potency is at maximum and component is enabled.
    pub fn is_developed(&self) -> bool {
        self.potency >= self.max_potency && self.enabled
    }

    /// `true` when potency is 0 (not gated by `enabled`).
    pub fn is_arrested(&self) -> bool {
        self.potency == 0.0
    }

    /// Fraction of maximum potency [0.0, 1.0].
    pub fn potency_fraction(&self) -> f32 {
        (self.potency / self.max_potency).clamp(0.0, 1.0)
    }

    /// Returns `scale * potency_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_vitality(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.potency_fraction()
    }
}

impl Default for Zygote {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zygote {
        Zygote::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_arrested() {
        let z = z();
        assert_eq!(z.potency, 0.0);
        assert!(z.is_arrested());
        assert!(!z.is_developed());
    }

    #[test]
    fn new_clamps_max_potency() {
        let z = Zygote::new(-5.0, 2.0);
        assert!((z.max_potency - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_division_rate() {
        let z = Zygote::new(100.0, -2.0);
        assert_eq!(z.division_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zygote::default();
        assert!((z.max_potency - 100.0).abs() < 1e-5);
        assert!((z.division_rate - 2.0).abs() < 1e-5);
    }

    // --- fuse ---

    #[test]
    fn fuse_adds_potency() {
        let mut z = z();
        z.fuse(40.0);
        assert!((z.potency - 40.0).abs() < 1e-3);
    }

    #[test]
    fn fuse_clamps_at_max() {
        let mut z = z();
        z.fuse(200.0);
        assert!((z.potency - 100.0).abs() < 1e-3);
    }

    #[test]
    fn fuse_fires_just_developed_at_max() {
        let mut z = z();
        z.fuse(100.0);
        assert!(z.just_developed);
        assert!(z.is_developed());
    }

    #[test]
    fn fuse_no_just_developed_when_already_at_max() {
        let mut z = z();
        z.potency = 100.0;
        z.fuse(10.0);
        assert!(!z.just_developed);
    }

    #[test]
    fn fuse_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.fuse(50.0);
        assert_eq!(z.potency, 0.0);
    }

    #[test]
    fn fuse_no_op_when_amount_zero() {
        let mut z = z();
        z.fuse(0.0);
        assert_eq!(z.potency, 0.0);
    }

    // --- arrest ---

    #[test]
    fn arrest_reduces_potency() {
        let mut z = z();
        z.potency = 60.0;
        z.arrest(20.0);
        assert!((z.potency - 40.0).abs() < 1e-3);
    }

    #[test]
    fn arrest_clamps_at_zero() {
        let mut z = z();
        z.potency = 30.0;
        z.arrest(200.0);
        assert_eq!(z.potency, 0.0);
    }

    #[test]
    fn arrest_fires_just_arrested_at_zero() {
        let mut z = z();
        z.potency = 30.0;
        z.arrest(30.0);
        assert!(z.just_arrested);
    }

    #[test]
    fn arrest_no_op_when_already_arrested() {
        let mut z = z();
        z.arrest(10.0);
        assert!(!z.just_arrested);
    }

    #[test]
    fn arrest_no_op_when_disabled() {
        let mut z = z();
        z.potency = 50.0;
        z.enabled = false;
        z.arrest(50.0);
        assert!((z.potency - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_divides_potency() {
        let mut z = z(); // rate=2
        z.tick(3.0); // 0 + 2*3 = 6
        assert!((z.potency - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_developed_on_divide_to_max() {
        let mut z = Zygote::new(100.0, 200.0);
        z.potency = 95.0;
        z.tick(1.0);
        assert!(z.just_developed);
        assert!(z.is_developed());
    }

    #[test]
    fn tick_no_divide_when_already_developed() {
        let mut z = z();
        z.potency = 100.0;
        z.tick(1.0);
        assert!(!z.just_developed);
    }

    #[test]
    fn tick_no_divide_when_rate_zero() {
        let mut z = Zygote::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.potency, 0.0);
    }

    #[test]
    fn tick_no_divide_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.potency, 0.0);
    }

    #[test]
    fn tick_clears_just_developed() {
        let mut z = Zygote::new(100.0, 200.0);
        z.potency = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_developed);
    }

    #[test]
    fn tick_clears_just_arrested() {
        let mut z = z();
        z.potency = 10.0;
        z.arrest(10.0);
        z.tick(0.016);
        assert!(!z.just_arrested);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2
        z.tick(5.0); // 2*5 = 10
        assert!((z.potency - 10.0).abs() < 1e-3);
    }

    // --- is_developed / is_arrested ---

    #[test]
    fn is_developed_false_when_disabled() {
        let mut z = z();
        z.potency = 100.0;
        z.enabled = false;
        assert!(!z.is_developed());
    }

    #[test]
    fn is_arrested_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_arrested());
    }

    // --- potency_fraction / effective_vitality ---

    #[test]
    fn potency_fraction_zero_when_arrested() {
        assert_eq!(z().potency_fraction(), 0.0);
    }

    #[test]
    fn potency_fraction_half_at_midpoint() {
        let mut z = z();
        z.potency = 50.0;
        assert!((z.potency_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_vitality_zero_when_arrested() {
        assert_eq!(z().effective_vitality(100.0), 0.0);
    }

    #[test]
    fn effective_vitality_scales_with_potency() {
        let mut z = z();
        z.potency = 75.0;
        assert!((z.effective_vitality(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_vitality_zero_when_disabled() {
        let mut z = z();
        z.potency = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_vitality(100.0), 0.0);
    }
}
