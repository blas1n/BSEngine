use bevy_ecs::prelude::Component;

/// Devotion-intensity tracker. `fervor` builds via `devote(amount)` and
/// rises passively at `zeal_rate` per second in `tick(dt)` or drops
/// immediately via `waver(amount)`.
///
/// Models fanaticism meters, conviction gauges, cult-loyalty trackers,
/// religious-fervour scales, berserk-devotion bars, or any mechanic where
/// a character or faction becomes increasingly committed to a cause the
/// longer they pursue it — and loses that fire only when directly shaken.
///
/// `devote(amount)` adds fervor; fires `just_zealous` when first reaching
/// `max_fervor`. No-op when disabled.
///
/// `waver(amount)` reduces fervor immediately; fires `just_lapsed` when
/// reaching 0. No-op when disabled or already lapsed.
///
/// `tick(dt)` clears both flags, then fans fervor by `zeal_rate * dt`
/// (capped at `max_fervor`). Fires `just_zealous` when first reaching max.
/// No-op when disabled or rate is 0.
///
/// `is_zealous()` returns `fervor >= max_fervor && enabled`.
///
/// `is_lapsed()` returns `fervor == 0.0` (not gated by `enabled`).
///
/// `fervor_fraction()` returns `(fervor / max_fervor).clamp(0, 1)`.
///
/// `effective_devotion(scale)` returns `scale * fervor_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 8.0)` — fans at 8 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zealot {
    pub fervor: f32,
    pub max_fervor: f32,
    pub zeal_rate: f32,
    pub just_zealous: bool,
    pub just_lapsed: bool,
    pub enabled: bool,
}

impl Zealot {
    pub fn new(max_fervor: f32, zeal_rate: f32) -> Self {
        Self {
            fervor: 0.0,
            max_fervor: max_fervor.max(0.1),
            zeal_rate: zeal_rate.max(0.0),
            just_zealous: false,
            just_lapsed: false,
            enabled: true,
        }
    }

    /// Add fervor; fires `just_zealous` when first reaching max.
    /// No-op when disabled.
    pub fn devote(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.fervor < self.max_fervor;
        self.fervor = (self.fervor + amount).min(self.max_fervor);
        if was_below && self.fervor >= self.max_fervor {
            self.just_zealous = true;
        }
    }

    /// Reduce fervor; fires `just_lapsed` when reaching 0.
    /// No-op when disabled or already lapsed.
    pub fn waver(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.fervor <= 0.0 {
            return;
        }
        self.fervor = (self.fervor - amount).max(0.0);
        if self.fervor <= 0.0 {
            self.just_lapsed = true;
        }
    }

    /// Clear flags, then fan fervor by `zeal_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_zealous = false;
        self.just_lapsed = false;
        if self.enabled && self.zeal_rate > 0.0 && self.fervor < self.max_fervor {
            let was_below = self.fervor < self.max_fervor;
            self.fervor = (self.fervor + self.zeal_rate * dt).min(self.max_fervor);
            if was_below && self.fervor >= self.max_fervor {
                self.just_zealous = true;
            }
        }
    }

    /// `true` when fervor is at maximum and component is enabled.
    pub fn is_zealous(&self) -> bool {
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
    pub fn effective_devotion(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.fervor_fraction()
    }
}

impl Default for Zealot {
    fn default() -> Self {
        Self::new(100.0, 8.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zealot {
        Zealot::new(100.0, 8.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_lapsed() {
        let z = z();
        assert_eq!(z.fervor, 0.0);
        assert!(z.is_lapsed());
        assert!(!z.is_zealous());
    }

    #[test]
    fn new_clamps_max_fervor() {
        let z = Zealot::new(-5.0, 8.0);
        assert!((z.max_fervor - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_zeal_rate() {
        let z = Zealot::new(100.0, -3.0);
        assert_eq!(z.zeal_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zealot::default();
        assert!((z.max_fervor - 100.0).abs() < 1e-5);
        assert!((z.zeal_rate - 8.0).abs() < 1e-5);
    }

    // --- devote ---

    #[test]
    fn devote_adds_fervor() {
        let mut z = z();
        z.devote(40.0);
        assert!((z.fervor - 40.0).abs() < 1e-3);
    }

    #[test]
    fn devote_clamps_at_max() {
        let mut z = z();
        z.devote(200.0);
        assert!((z.fervor - 100.0).abs() < 1e-3);
    }

    #[test]
    fn devote_fires_just_zealous_at_max() {
        let mut z = z();
        z.devote(100.0);
        assert!(z.just_zealous);
        assert!(z.is_zealous());
    }

    #[test]
    fn devote_no_just_zealous_when_already_at_max() {
        let mut z = z();
        z.fervor = 100.0;
        z.devote(10.0);
        assert!(!z.just_zealous);
    }

    #[test]
    fn devote_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.devote(50.0);
        assert_eq!(z.fervor, 0.0);
    }

    #[test]
    fn devote_no_op_when_amount_zero() {
        let mut z = z();
        z.devote(0.0);
        assert_eq!(z.fervor, 0.0);
    }

    // --- waver ---

    #[test]
    fn waver_reduces_fervor() {
        let mut z = z();
        z.fervor = 60.0;
        z.waver(20.0);
        assert!((z.fervor - 40.0).abs() < 1e-3);
    }

    #[test]
    fn waver_clamps_at_zero() {
        let mut z = z();
        z.fervor = 30.0;
        z.waver(200.0);
        assert_eq!(z.fervor, 0.0);
    }

    #[test]
    fn waver_fires_just_lapsed_at_zero() {
        let mut z = z();
        z.fervor = 30.0;
        z.waver(30.0);
        assert!(z.just_lapsed);
    }

    #[test]
    fn waver_no_op_when_already_lapsed() {
        let mut z = z();
        z.waver(10.0);
        assert!(!z.just_lapsed);
    }

    #[test]
    fn waver_no_op_when_disabled() {
        let mut z = z();
        z.fervor = 50.0;
        z.enabled = false;
        z.waver(50.0);
        assert!((z.fervor - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_fans_fervor() {
        let mut z = z(); // zeal=8
        z.tick(1.0); // 0 + 8 = 8
        assert!((z.fervor - 8.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_zealous_on_fan_to_max() {
        let mut z = Zealot::new(100.0, 200.0);
        z.fervor = 95.0;
        z.tick(1.0);
        assert!(z.just_zealous);
        assert!(z.is_zealous());
    }

    #[test]
    fn tick_no_fan_when_already_at_max() {
        let mut z = z();
        z.fervor = 100.0;
        z.tick(1.0);
        assert!(!z.just_zealous);
    }

    #[test]
    fn tick_no_fan_when_rate_zero() {
        let mut z = Zealot::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.fervor, 0.0);
    }

    #[test]
    fn tick_no_fan_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.fervor, 0.0);
    }

    #[test]
    fn tick_clears_just_zealous() {
        let mut z = Zealot::new(100.0, 200.0);
        z.fervor = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_zealous);
    }

    #[test]
    fn tick_clears_just_lapsed() {
        let mut z = z();
        z.fervor = 10.0;
        z.waver(10.0);
        z.tick(0.016);
        assert!(!z.just_lapsed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // zeal=8
        z.tick(4.0); // 8*4 = 32
        assert!((z.fervor - 32.0).abs() < 1e-3);
    }

    // --- is_zealous / is_lapsed ---

    #[test]
    fn is_zealous_false_when_disabled() {
        let mut z = z();
        z.fervor = 100.0;
        z.enabled = false;
        assert!(!z.is_zealous());
    }

    #[test]
    fn is_lapsed_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_lapsed());
    }

    // --- fervor_fraction / effective_devotion ---

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
    fn effective_devotion_zero_when_lapsed() {
        assert_eq!(z().effective_devotion(100.0), 0.0);
    }

    #[test]
    fn effective_devotion_scales_with_fervor() {
        let mut z = z();
        z.fervor = 75.0;
        assert!((z.effective_devotion(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_devotion_zero_when_disabled() {
        let mut z = z();
        z.fervor = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_devotion(100.0), 0.0);
    }
}
