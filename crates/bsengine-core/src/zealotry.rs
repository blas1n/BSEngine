use bevy_ecs::prelude::Component;

/// Fanaticism-fervor tracker. `fervor` builds via `radicalize(amount)`
/// and intensifies passively at `dogma_rate` per second in `tick(dt)`
/// or is tempered immediately via `moderate(amount)`.
///
/// Models religious-fanaticism escalation meters, cult-indoctrination bars,
/// ideological-radicalization accumulators, true-believer fervor gauges,
/// crusade-zeal build-up trackers, faction-extremism fill levels,
/// revolutionary-ardour intensity indicators, martyr-complex progress bars,
/// doctrinal-purity scoring meters, or any mechanic where absolute
/// conviction in a cause overtakes reason — until a single moderating voice
/// breaks the spell and the fervor collapses back to nothing.
///
/// `radicalize(amount)` adds fervor; fires `just_fanatical` when first
/// reaching `max_fervor`. No-op when disabled.
///
/// `moderate(amount)` reduces fervor immediately; fires `just_lapsed` when
/// reaching 0. No-op when disabled or already lapsed.
///
/// `tick(dt)` clears both flags, then increases fervor by
/// `dogma_rate * dt` (capped at `max_fervor`). Fires `just_fanatical`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_fanatical()` returns `fervor >= max_fervor && enabled`.
///
/// `is_lapsed()` returns `fervor == 0.0` (not gated by `enabled`).
///
/// `fervor_fraction()` returns `(fervor / max_fervor).clamp(0, 1)`.
///
/// `effective_dogma(scale)` returns `scale * fervor_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — radicalises at 2 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zealotry {
    pub fervor: f32,
    pub max_fervor: f32,
    pub dogma_rate: f32,
    pub just_fanatical: bool,
    pub just_lapsed: bool,
    pub enabled: bool,
}

impl Zealotry {
    pub fn new(max_fervor: f32, dogma_rate: f32) -> Self {
        Self {
            fervor: 0.0,
            max_fervor: max_fervor.max(0.1),
            dogma_rate: dogma_rate.max(0.0),
            just_fanatical: false,
            just_lapsed: false,
            enabled: true,
        }
    }

    /// Add fervor; fires `just_fanatical` when first reaching max.
    /// No-op when disabled.
    pub fn radicalize(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.fervor < self.max_fervor;
        self.fervor = (self.fervor + amount).min(self.max_fervor);
        if was_below && self.fervor >= self.max_fervor {
            self.just_fanatical = true;
        }
    }

    /// Reduce fervor; fires `just_lapsed` when reaching 0.
    /// No-op when disabled or already lapsed.
    pub fn moderate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.fervor <= 0.0 {
            return;
        }
        self.fervor = (self.fervor - amount).max(0.0);
        if self.fervor <= 0.0 {
            self.just_lapsed = true;
        }
    }

    /// Clear flags, then increase fervor by `dogma_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_fanatical = false;
        self.just_lapsed = false;
        if self.enabled && self.dogma_rate > 0.0 && self.fervor < self.max_fervor {
            let was_below = self.fervor < self.max_fervor;
            self.fervor = (self.fervor + self.dogma_rate * dt).min(self.max_fervor);
            if was_below && self.fervor >= self.max_fervor {
                self.just_fanatical = true;
            }
        }
    }

    /// `true` when fervor is at maximum and component is enabled.
    pub fn is_fanatical(&self) -> bool {
        self.fervor >= self.max_fervor && self.enabled
    }

    /// `true` when fervor is 0 (not gated by `enabled`).
    pub fn is_lapsed(&self) -> bool {
        self.fervor == 0.0
    }

    /// Fraction of maximum fervor [0.0, 1.0].
    pub fn fervor_fraction(&self) -> f32 {
        (self.fervor / self.max_fervor).clamp(0.0, 1.0)
    }

    /// Returns `scale * fervor_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_dogma(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.fervor_fraction()
    }
}

impl Default for Zealotry {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zealotry {
        Zealotry::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_lapsed() {
        let z = z();
        assert_eq!(z.fervor, 0.0);
        assert!(z.is_lapsed());
        assert!(!z.is_fanatical());
    }

    #[test]
    fn new_clamps_max_fervor() {
        let z = Zealotry::new(-5.0, 2.0);
        assert!((z.max_fervor - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_dogma_rate() {
        let z = Zealotry::new(100.0, -3.0);
        assert_eq!(z.dogma_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zealotry::default();
        assert!((z.max_fervor - 100.0).abs() < 1e-5);
        assert!((z.dogma_rate - 2.0).abs() < 1e-5);
    }

    // --- radicalize ---

    #[test]
    fn radicalize_adds_fervor() {
        let mut z = z();
        z.radicalize(40.0);
        assert!((z.fervor - 40.0).abs() < 1e-3);
    }

    #[test]
    fn radicalize_clamps_at_max() {
        let mut z = z();
        z.radicalize(200.0);
        assert!((z.fervor - 100.0).abs() < 1e-3);
    }

    #[test]
    fn radicalize_fires_just_fanatical_at_max() {
        let mut z = z();
        z.radicalize(100.0);
        assert!(z.just_fanatical);
        assert!(z.is_fanatical());
    }

    #[test]
    fn radicalize_no_just_fanatical_when_already_at_max() {
        let mut z = z();
        z.fervor = 100.0;
        z.radicalize(10.0);
        assert!(!z.just_fanatical);
    }

    #[test]
    fn radicalize_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.radicalize(50.0);
        assert_eq!(z.fervor, 0.0);
    }

    #[test]
    fn radicalize_no_op_when_amount_zero() {
        let mut z = z();
        z.radicalize(0.0);
        assert_eq!(z.fervor, 0.0);
    }

    // --- moderate ---

    #[test]
    fn moderate_reduces_fervor() {
        let mut z = z();
        z.fervor = 60.0;
        z.moderate(20.0);
        assert!((z.fervor - 40.0).abs() < 1e-3);
    }

    #[test]
    fn moderate_clamps_at_zero() {
        let mut z = z();
        z.fervor = 30.0;
        z.moderate(200.0);
        assert_eq!(z.fervor, 0.0);
    }

    #[test]
    fn moderate_fires_just_lapsed_at_zero() {
        let mut z = z();
        z.fervor = 30.0;
        z.moderate(30.0);
        assert!(z.just_lapsed);
    }

    #[test]
    fn moderate_no_op_when_already_lapsed() {
        let mut z = z();
        z.moderate(10.0);
        assert!(!z.just_lapsed);
    }

    #[test]
    fn moderate_no_op_when_disabled() {
        let mut z = z();
        z.fervor = 50.0;
        z.enabled = false;
        z.moderate(50.0);
        assert!((z.fervor - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_radicalises_fervor() {
        let mut z = z(); // rate=2
        z.tick(3.0); // 0 + 2*3 = 6
        assert!((z.fervor - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_fanatical_on_dogma_to_max() {
        let mut z = Zealotry::new(100.0, 200.0);
        z.fervor = 95.0;
        z.tick(1.0);
        assert!(z.just_fanatical);
        assert!(z.is_fanatical());
    }

    #[test]
    fn tick_no_dogma_when_already_fanatical() {
        let mut z = z();
        z.fervor = 100.0;
        z.tick(1.0);
        assert!(!z.just_fanatical);
    }

    #[test]
    fn tick_no_dogma_when_rate_zero() {
        let mut z = Zealotry::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.fervor, 0.0);
    }

    #[test]
    fn tick_no_dogma_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.fervor, 0.0);
    }

    #[test]
    fn tick_clears_just_fanatical() {
        let mut z = Zealotry::new(100.0, 200.0);
        z.fervor = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_fanatical);
    }

    #[test]
    fn tick_clears_just_lapsed() {
        let mut z = z();
        z.fervor = 10.0;
        z.moderate(10.0);
        z.tick(0.016);
        assert!(!z.just_lapsed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2
        z.tick(5.0); // 2*5 = 10
        assert!((z.fervor - 10.0).abs() < 1e-3);
    }

    // --- is_fanatical / is_lapsed ---

    #[test]
    fn is_fanatical_false_when_disabled() {
        let mut z = z();
        z.fervor = 100.0;
        z.enabled = false;
        assert!(!z.is_fanatical());
    }

    #[test]
    fn is_lapsed_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_lapsed());
    }

    // --- fervor_fraction / effective_dogma ---

    #[test]
    fn fervor_fraction_zero_when_lapsed() {
        assert_eq!(z().fervor_fraction(), 0.0);
    }

    #[test]
    fn fervor_fraction_half_at_midpoint() {
        let mut z = z();
        z.fervor = 50.0;
        assert!((z.fervor_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_dogma_zero_when_lapsed() {
        assert_eq!(z().effective_dogma(100.0), 0.0);
    }

    #[test]
    fn effective_dogma_scales_with_fervor() {
        let mut z = z();
        z.fervor = 80.0;
        assert!((z.effective_dogma(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_dogma_zero_when_disabled() {
        let mut z = z();
        z.fervor = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_dogma(100.0), 0.0);
    }
}
