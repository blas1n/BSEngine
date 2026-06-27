use bevy_ecs::prelude::Component;

/// Lingering-affliction tracker. `linger` builds via `inflame(amount)`
/// and persists passively at `linger_rate` per second in `tick(dt)` or
/// is soothed immediately via `soothe(amount)`.
///
/// Models herpes-zoster recurrence meters, persistent-nerve-pain
/// accumulation bars, sea-grass coverage fill levels, lingering-ailment
/// duration gauges, chronic-condition flare-up trackers, shingles-outbreak
/// intensity indicators, dormant-pathogen activation meters, recurring-rash
/// severity accumulators, or any mechanic where a latent affliction
/// slowly builds until it erupts into a full-blown outbreak.
///
/// `inflame(amount)` adds linger; fires `just_erupted` when first
/// reaching `max_linger`. No-op when disabled.
///
/// `soothe(amount)` reduces linger immediately; fires `just_dormant`
/// when reaching 0. No-op when disabled or already dormant.
///
/// `tick(dt)` clears both flags, then increases linger by
/// `linger_rate * dt` (capped at `max_linger`). Fires `just_erupted`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_erupted()` returns `linger >= max_linger && enabled`.
///
/// `is_dormant()` returns `linger == 0.0` (not gated by `enabled`).
///
/// `linger_fraction()` returns `(linger / max_linger).clamp(0, 1)`.
///
/// `effective_severity(scale)` returns `scale * linger_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 0.5)` — lingers at 0.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoster {
    pub linger: f32,
    pub max_linger: f32,
    pub linger_rate: f32,
    pub just_erupted: bool,
    pub just_dormant: bool,
    pub enabled: bool,
}

impl Zoster {
    pub fn new(max_linger: f32, linger_rate: f32) -> Self {
        Self {
            linger: 0.0,
            max_linger: max_linger.max(0.1),
            linger_rate: linger_rate.max(0.0),
            just_erupted: false,
            just_dormant: false,
            enabled: true,
        }
    }

    /// Add linger; fires `just_erupted` when first reaching max.
    /// No-op when disabled.
    pub fn inflame(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.linger < self.max_linger;
        self.linger = (self.linger + amount).min(self.max_linger);
        if was_below && self.linger >= self.max_linger {
            self.just_erupted = true;
        }
    }

    /// Reduce linger; fires `just_dormant` when reaching 0.
    /// No-op when disabled or already dormant.
    pub fn soothe(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.linger <= 0.0 {
            return;
        }
        self.linger = (self.linger - amount).max(0.0);
        if self.linger <= 0.0 {
            self.just_dormant = true;
        }
    }

    /// Clear flags, then increase linger by `linger_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_erupted = false;
        self.just_dormant = false;
        if self.enabled && self.linger_rate > 0.0 && self.linger < self.max_linger {
            let was_below = self.linger < self.max_linger;
            self.linger = (self.linger + self.linger_rate * dt).min(self.max_linger);
            if was_below && self.linger >= self.max_linger {
                self.just_erupted = true;
            }
        }
    }

    /// `true` when linger is at maximum and component is enabled.
    pub fn is_erupted(&self) -> bool {
        self.linger >= self.max_linger && self.enabled
    }

    /// `true` when linger is 0 (not gated by `enabled`).
    pub fn is_dormant(&self) -> bool {
        self.linger == 0.0
    }

    /// Fraction of maximum linger [0.0, 1.0].
    pub fn linger_fraction(&self) -> f32 {
        (self.linger / self.max_linger).clamp(0.0, 1.0)
    }

    /// Returns `scale * linger_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_severity(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.linger_fraction()
    }
}

impl Default for Zoster {
    fn default() -> Self {
        Self::new(100.0, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoster {
        Zoster::new(100.0, 0.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_dormant() {
        let z = z();
        assert_eq!(z.linger, 0.0);
        assert!(z.is_dormant());
        assert!(!z.is_erupted());
    }

    #[test]
    fn new_clamps_max_linger() {
        let z = Zoster::new(-5.0, 0.5);
        assert!((z.max_linger - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_linger_rate() {
        let z = Zoster::new(100.0, -3.0);
        assert_eq!(z.linger_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoster::default();
        assert!((z.max_linger - 100.0).abs() < 1e-5);
        assert!((z.linger_rate - 0.5).abs() < 1e-5);
    }

    // --- inflame ---

    #[test]
    fn inflame_adds_linger() {
        let mut z = z();
        z.inflame(40.0);
        assert!((z.linger - 40.0).abs() < 1e-3);
    }

    #[test]
    fn inflame_clamps_at_max() {
        let mut z = z();
        z.inflame(200.0);
        assert!((z.linger - 100.0).abs() < 1e-3);
    }

    #[test]
    fn inflame_fires_just_erupted_at_max() {
        let mut z = z();
        z.inflame(100.0);
        assert!(z.just_erupted);
        assert!(z.is_erupted());
    }

    #[test]
    fn inflame_no_just_erupted_when_already_at_max() {
        let mut z = z();
        z.linger = 100.0;
        z.inflame(10.0);
        assert!(!z.just_erupted);
    }

    #[test]
    fn inflame_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.inflame(50.0);
        assert_eq!(z.linger, 0.0);
    }

    #[test]
    fn inflame_no_op_when_amount_zero() {
        let mut z = z();
        z.inflame(0.0);
        assert_eq!(z.linger, 0.0);
    }

    // --- soothe ---

    #[test]
    fn soothe_reduces_linger() {
        let mut z = z();
        z.linger = 60.0;
        z.soothe(20.0);
        assert!((z.linger - 40.0).abs() < 1e-3);
    }

    #[test]
    fn soothe_clamps_at_zero() {
        let mut z = z();
        z.linger = 30.0;
        z.soothe(200.0);
        assert_eq!(z.linger, 0.0);
    }

    #[test]
    fn soothe_fires_just_dormant_at_zero() {
        let mut z = z();
        z.linger = 30.0;
        z.soothe(30.0);
        assert!(z.just_dormant);
    }

    #[test]
    fn soothe_no_op_when_already_dormant() {
        let mut z = z();
        z.soothe(10.0);
        assert!(!z.just_dormant);
    }

    #[test]
    fn soothe_no_op_when_disabled() {
        let mut z = z();
        z.linger = 50.0;
        z.enabled = false;
        z.soothe(50.0);
        assert!((z.linger - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_increases_linger() {
        let mut z = z(); // rate=0.5
        z.tick(2.0); // 0 + 0.5*2 = 1
        assert!((z.linger - 1.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_erupted_on_linger_to_max() {
        let mut z = Zoster::new(100.0, 200.0);
        z.linger = 95.0;
        z.tick(1.0);
        assert!(z.just_erupted);
        assert!(z.is_erupted());
    }

    #[test]
    fn tick_no_linger_when_already_erupted() {
        let mut z = z();
        z.linger = 100.0;
        z.tick(1.0);
        assert!(!z.just_erupted);
    }

    #[test]
    fn tick_no_linger_when_rate_zero() {
        let mut z = Zoster::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.linger, 0.0);
    }

    #[test]
    fn tick_no_linger_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.linger, 0.0);
    }

    #[test]
    fn tick_clears_just_erupted() {
        let mut z = Zoster::new(100.0, 200.0);
        z.linger = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_erupted);
    }

    #[test]
    fn tick_clears_just_dormant() {
        let mut z = z();
        z.linger = 10.0;
        z.soothe(10.0);
        z.tick(0.016);
        assert!(!z.just_dormant);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=0.5
        z.tick(10.0); // 0.5*10 = 5
        assert!((z.linger - 5.0).abs() < 1e-3);
    }

    // --- is_erupted / is_dormant ---

    #[test]
    fn is_erupted_false_when_disabled() {
        let mut z = z();
        z.linger = 100.0;
        z.enabled = false;
        assert!(!z.is_erupted());
    }

    #[test]
    fn is_dormant_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_dormant());
    }

    // --- linger_fraction / effective_severity ---

    #[test]
    fn linger_fraction_zero_when_dormant() {
        assert_eq!(z().linger_fraction(), 0.0);
    }

    #[test]
    fn linger_fraction_half_at_midpoint() {
        let mut z = z();
        z.linger = 50.0;
        assert!((z.linger_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_severity_zero_when_dormant() {
        assert_eq!(z().effective_severity(100.0), 0.0);
    }

    #[test]
    fn effective_severity_scales_with_linger() {
        let mut z = z();
        z.linger = 60.0;
        assert!((z.effective_severity(100.0) - 60.0).abs() < 1e-3);
    }

    #[test]
    fn effective_severity_zero_when_disabled() {
        let mut z = z();
        z.linger = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_severity(100.0), 0.0);
    }
}
