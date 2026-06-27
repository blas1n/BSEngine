use bevy_ecs::prelude::Component;

/// Purification-progress tracker. `purity` builds via `purify(amount)`
/// and is restored passively at `cleanse_rate` per second in `tick(dt)`
/// or contaminated immediately via `contaminate(amount)`.
///
/// Models detoxification meters, water-quality gauges, poison-purging
/// progress, magical-corruption clears, filter-saturation trackers, or
/// any mechanic where a substance or entity gradually becomes cleaner
/// when treated, and dirtier when exposed.
///
/// `purify(amount)` adds purity; fires `just_cleansed` when first
/// reaching `max_purity`. No-op when disabled.
///
/// `contaminate(amount)` reduces purity immediately; fires `just_fouled`
/// when reaching 0. No-op when disabled or already fouled.
///
/// `tick(dt)` clears both flags, then cleanses purity by
/// `cleanse_rate * dt` (capped at `max_purity`). Fires `just_cleansed`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_cleansed()` returns `purity >= max_purity && enabled`.
///
/// `is_fouled()` returns `purity == 0.0` (not gated by `enabled`).
///
/// `purity_fraction()` returns `(purity / max_purity).clamp(0, 1)`.
///
/// `effective_filtration(scale)` returns `scale * purity_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 7.0)` — cleanses at 7 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zeolite {
    pub purity: f32,
    pub max_purity: f32,
    pub cleanse_rate: f32,
    pub just_cleansed: bool,
    pub just_fouled: bool,
    pub enabled: bool,
}

impl Zeolite {
    pub fn new(max_purity: f32, cleanse_rate: f32) -> Self {
        Self {
            purity: 0.0,
            max_purity: max_purity.max(0.1),
            cleanse_rate: cleanse_rate.max(0.0),
            just_cleansed: false,
            just_fouled: false,
            enabled: true,
        }
    }

    /// Add purity; fires `just_cleansed` when first reaching max.
    /// No-op when disabled.
    pub fn purify(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.purity < self.max_purity;
        self.purity = (self.purity + amount).min(self.max_purity);
        if was_below && self.purity >= self.max_purity {
            self.just_cleansed = true;
        }
    }

    /// Reduce purity; fires `just_fouled` when reaching 0.
    /// No-op when disabled or already fouled.
    pub fn contaminate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.purity <= 0.0 {
            return;
        }
        self.purity = (self.purity - amount).max(0.0);
        if self.purity <= 0.0 {
            self.just_fouled = true;
        }
    }

    /// Clear flags, then cleanse purity by `cleanse_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_cleansed = false;
        self.just_fouled = false;
        if self.enabled && self.cleanse_rate > 0.0 && self.purity < self.max_purity {
            let was_below = self.purity < self.max_purity;
            self.purity = (self.purity + self.cleanse_rate * dt).min(self.max_purity);
            if was_below && self.purity >= self.max_purity {
                self.just_cleansed = true;
            }
        }
    }

    /// `true` when purity is at maximum and component is enabled.
    pub fn is_cleansed(&self) -> bool {
        self.purity >= self.max_purity && self.enabled
    }

    /// `true` when purity is 0 (not gated by `enabled`).
    pub fn is_fouled(&self) -> bool {
        self.purity == 0.0
    }

    /// Fraction of maximum purity [0.0, 1.0].
    pub fn purity_fraction(&self) -> f32 {
        (self.purity / self.max_purity).clamp(0.0, 1.0)
    }

    /// Returns `scale * purity_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_filtration(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.purity_fraction()
    }
}

impl Default for Zeolite {
    fn default() -> Self {
        Self::new(100.0, 7.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zeolite {
        Zeolite::new(100.0, 7.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_fouled() {
        let z = z();
        assert_eq!(z.purity, 0.0);
        assert!(z.is_fouled());
        assert!(!z.is_cleansed());
    }

    #[test]
    fn new_clamps_max_purity() {
        let z = Zeolite::new(-5.0, 7.0);
        assert!((z.max_purity - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_cleanse_rate() {
        let z = Zeolite::new(100.0, -3.0);
        assert_eq!(z.cleanse_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zeolite::default();
        assert!((z.max_purity - 100.0).abs() < 1e-5);
        assert!((z.cleanse_rate - 7.0).abs() < 1e-5);
    }

    // --- purify ---

    #[test]
    fn purify_adds_purity() {
        let mut z = z();
        z.purify(40.0);
        assert!((z.purity - 40.0).abs() < 1e-3);
    }

    #[test]
    fn purify_clamps_at_max() {
        let mut z = z();
        z.purify(200.0);
        assert!((z.purity - 100.0).abs() < 1e-3);
    }

    #[test]
    fn purify_fires_just_cleansed_at_max() {
        let mut z = z();
        z.purify(100.0);
        assert!(z.just_cleansed);
        assert!(z.is_cleansed());
    }

    #[test]
    fn purify_no_just_cleansed_when_already_at_max() {
        let mut z = z();
        z.purity = 100.0;
        z.purify(10.0);
        assert!(!z.just_cleansed);
    }

    #[test]
    fn purify_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.purify(50.0);
        assert_eq!(z.purity, 0.0);
    }

    #[test]
    fn purify_no_op_when_amount_zero() {
        let mut z = z();
        z.purify(0.0);
        assert_eq!(z.purity, 0.0);
    }

    // --- contaminate ---

    #[test]
    fn contaminate_reduces_purity() {
        let mut z = z();
        z.purity = 60.0;
        z.contaminate(20.0);
        assert!((z.purity - 40.0).abs() < 1e-3);
    }

    #[test]
    fn contaminate_clamps_at_zero() {
        let mut z = z();
        z.purity = 30.0;
        z.contaminate(200.0);
        assert_eq!(z.purity, 0.0);
    }

    #[test]
    fn contaminate_fires_just_fouled_at_zero() {
        let mut z = z();
        z.purity = 30.0;
        z.contaminate(30.0);
        assert!(z.just_fouled);
    }

    #[test]
    fn contaminate_no_op_when_already_fouled() {
        let mut z = z();
        z.contaminate(10.0);
        assert!(!z.just_fouled);
    }

    #[test]
    fn contaminate_no_op_when_disabled() {
        let mut z = z();
        z.purity = 50.0;
        z.enabled = false;
        z.contaminate(50.0);
        assert!((z.purity - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_cleanses_purity() {
        let mut z = z(); // cleanse=7
        z.purity = 50.0;
        z.tick(1.0); // 50 + 7 = 57
        assert!((z.purity - 57.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_cleansed_on_cleanse_to_max() {
        let mut z = Zeolite::new(100.0, 200.0);
        z.purity = 95.0;
        z.tick(1.0);
        assert!(z.just_cleansed);
        assert!(z.is_cleansed());
    }

    #[test]
    fn tick_no_cleanse_when_already_at_max() {
        let mut z = z();
        z.purity = 100.0;
        z.tick(1.0);
        assert!(!z.just_cleansed);
    }

    #[test]
    fn tick_no_cleanse_when_rate_zero() {
        let mut z = Zeolite::new(100.0, 0.0);
        z.purity = 50.0;
        z.tick(100.0);
        assert!((z.purity - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_cleanse_when_disabled() {
        let mut z = z();
        z.purity = 50.0;
        z.enabled = false;
        z.tick(1.0);
        assert!((z.purity - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_cleansed() {
        let mut z = Zeolite::new(100.0, 200.0);
        z.purity = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_cleansed);
    }

    #[test]
    fn tick_clears_just_fouled() {
        let mut z = z();
        z.purity = 10.0;
        z.contaminate(10.0);
        z.tick(0.016);
        assert!(!z.just_fouled);
    }

    #[test]
    fn tick_scales_cleanse_with_dt() {
        let mut z = z(); // cleanse=7
        z.tick(4.0); // 0 + 7*4 = 28
        assert!((z.purity - 28.0).abs() < 1e-3);
    }

    // --- is_cleansed / is_fouled ---

    #[test]
    fn is_cleansed_false_when_disabled() {
        let mut z = z();
        z.purity = 100.0;
        z.enabled = false;
        assert!(!z.is_cleansed());
    }

    #[test]
    fn is_fouled_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_fouled());
    }

    // --- purity_fraction / effective_filtration ---

    #[test]
    fn purity_fraction_zero_when_fouled() {
        assert_eq!(z().purity_fraction(), 0.0);
    }

    #[test]
    fn purity_fraction_half_at_midpoint() {
        let mut z = z();
        z.purity = 50.0;
        assert!((z.purity_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_filtration_zero_when_fouled() {
        assert_eq!(z().effective_filtration(100.0), 0.0);
    }

    #[test]
    fn effective_filtration_scales_with_purity() {
        let mut z = z();
        z.purity = 70.0;
        assert!((z.effective_filtration(100.0) - 70.0).abs() < 1e-3);
    }

    #[test]
    fn effective_filtration_zero_when_disabled() {
        let mut z = z();
        z.purity = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_filtration(100.0), 0.0);
    }
}
