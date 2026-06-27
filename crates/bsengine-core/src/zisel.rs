use bevy_ecs::prelude::Component;

/// Sentinel-vigilance tracker. `alert` builds via `spot(amount)` and
/// rises passively at `scan_rate` per second in `tick(dt)` or drops
/// immediately via `relax(amount)`.
///
/// Models European-ground-squirrel alarm-call meters, sentry-duty
/// awareness bars, perimeter-watch intensity accumulators, lookout-post
/// threat-level gauges, colony-watchman arousal trackers, grassland-rodent
/// sentinel fill levels, burrow-guard proximity indicators, prairie-dog
/// alert-contagion progress bars, steppe-scout vigilance meters, or any
/// mechanic where a small sentinel accumulates threat awareness until it
/// erupts into a piercing alarm call that sends the whole colony diving
/// for cover.
///
/// `spot(amount)` adds alert; fires `just_alarmed` when first reaching
/// `max_alert`. No-op when disabled.
///
/// `relax(amount)` reduces alert immediately; fires `just_calm` when
/// reaching 0. No-op when disabled or already calm.
///
/// `tick(dt)` clears both flags, then increases alert by
/// `scan_rate * dt` (capped at `max_alert`). Fires `just_alarmed` when
/// first reaching max. No-op when disabled or rate is 0.
///
/// `is_alarmed()` returns `alert >= max_alert && enabled`.
///
/// `is_calm()` returns `alert == 0.0` (not gated by `enabled`).
///
/// `alert_fraction()` returns `(alert / max_alert).clamp(0, 1)`.
///
/// `effective_vigilance(scale)` returns `scale * alert_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 3.0)` — scans at 3 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zisel {
    pub alert: f32,
    pub max_alert: f32,
    pub scan_rate: f32,
    pub just_alarmed: bool,
    pub just_calm: bool,
    pub enabled: bool,
}

impl Zisel {
    pub fn new(max_alert: f32, scan_rate: f32) -> Self {
        Self {
            alert: 0.0,
            max_alert: max_alert.max(0.1),
            scan_rate: scan_rate.max(0.0),
            just_alarmed: false,
            just_calm: false,
            enabled: true,
        }
    }

    /// Add alert; fires `just_alarmed` when first reaching max.
    /// No-op when disabled.
    pub fn spot(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.alert < self.max_alert;
        self.alert = (self.alert + amount).min(self.max_alert);
        if was_below && self.alert >= self.max_alert {
            self.just_alarmed = true;
        }
    }

    /// Reduce alert; fires `just_calm` when reaching 0.
    /// No-op when disabled or already calm.
    pub fn relax(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.alert <= 0.0 {
            return;
        }
        self.alert = (self.alert - amount).max(0.0);
        if self.alert <= 0.0 {
            self.just_calm = true;
        }
    }

    /// Clear flags, then increase alert by `scan_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_alarmed = false;
        self.just_calm = false;
        if self.enabled && self.scan_rate > 0.0 && self.alert < self.max_alert {
            let was_below = self.alert < self.max_alert;
            self.alert = (self.alert + self.scan_rate * dt).min(self.max_alert);
            if was_below && self.alert >= self.max_alert {
                self.just_alarmed = true;
            }
        }
    }

    /// `true` when alert is at maximum and component is enabled.
    pub fn is_alarmed(&self) -> bool {
        self.alert >= self.max_alert && self.enabled
    }

    /// `true` when alert is 0 (not gated by `enabled`).
    pub fn is_calm(&self) -> bool {
        self.alert == 0.0
    }

    /// Fraction of maximum alert [0.0, 1.0].
    pub fn alert_fraction(&self) -> f32 {
        (self.alert / self.max_alert).clamp(0.0, 1.0)
    }

    /// Returns `scale * alert_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_vigilance(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.alert_fraction()
    }
}

impl Default for Zisel {
    fn default() -> Self {
        Self::new(100.0, 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zisel {
        Zisel::new(100.0, 3.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_calm() {
        let z = z();
        assert_eq!(z.alert, 0.0);
        assert!(z.is_calm());
        assert!(!z.is_alarmed());
    }

    #[test]
    fn new_clamps_max_alert() {
        let z = Zisel::new(-5.0, 3.0);
        assert!((z.max_alert - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_scan_rate() {
        let z = Zisel::new(100.0, -3.0);
        assert_eq!(z.scan_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zisel::default();
        assert!((z.max_alert - 100.0).abs() < 1e-5);
        assert!((z.scan_rate - 3.0).abs() < 1e-5);
    }

    // --- spot ---

    #[test]
    fn spot_adds_alert() {
        let mut z = z();
        z.spot(40.0);
        assert!((z.alert - 40.0).abs() < 1e-3);
    }

    #[test]
    fn spot_clamps_at_max() {
        let mut z = z();
        z.spot(200.0);
        assert!((z.alert - 100.0).abs() < 1e-3);
    }

    #[test]
    fn spot_fires_just_alarmed_at_max() {
        let mut z = z();
        z.spot(100.0);
        assert!(z.just_alarmed);
        assert!(z.is_alarmed());
    }

    #[test]
    fn spot_no_just_alarmed_when_already_at_max() {
        let mut z = z();
        z.alert = 100.0;
        z.spot(10.0);
        assert!(!z.just_alarmed);
    }

    #[test]
    fn spot_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.spot(50.0);
        assert_eq!(z.alert, 0.0);
    }

    #[test]
    fn spot_no_op_when_amount_zero() {
        let mut z = z();
        z.spot(0.0);
        assert_eq!(z.alert, 0.0);
    }

    // --- relax ---

    #[test]
    fn relax_reduces_alert() {
        let mut z = z();
        z.alert = 60.0;
        z.relax(20.0);
        assert!((z.alert - 40.0).abs() < 1e-3);
    }

    #[test]
    fn relax_clamps_at_zero() {
        let mut z = z();
        z.alert = 30.0;
        z.relax(200.0);
        assert_eq!(z.alert, 0.0);
    }

    #[test]
    fn relax_fires_just_calm_at_zero() {
        let mut z = z();
        z.alert = 30.0;
        z.relax(30.0);
        assert!(z.just_calm);
    }

    #[test]
    fn relax_no_op_when_already_calm() {
        let mut z = z();
        z.relax(10.0);
        assert!(!z.just_calm);
    }

    #[test]
    fn relax_no_op_when_disabled() {
        let mut z = z();
        z.alert = 50.0;
        z.enabled = false;
        z.relax(50.0);
        assert!((z.alert - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_raises_alert() {
        let mut z = z(); // rate=3
        z.tick(2.0); // 0 + 3*2 = 6
        assert!((z.alert - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_alarmed_on_scan_to_max() {
        let mut z = Zisel::new(100.0, 200.0);
        z.alert = 95.0;
        z.tick(1.0);
        assert!(z.just_alarmed);
        assert!(z.is_alarmed());
    }

    #[test]
    fn tick_no_scan_when_already_alarmed() {
        let mut z = z();
        z.alert = 100.0;
        z.tick(1.0);
        assert!(!z.just_alarmed);
    }

    #[test]
    fn tick_no_scan_when_rate_zero() {
        let mut z = Zisel::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.alert, 0.0);
    }

    #[test]
    fn tick_no_scan_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.alert, 0.0);
    }

    #[test]
    fn tick_clears_just_alarmed() {
        let mut z = Zisel::new(100.0, 200.0);
        z.alert = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_alarmed);
    }

    #[test]
    fn tick_clears_just_calm() {
        let mut z = z();
        z.alert = 10.0;
        z.relax(10.0);
        z.tick(0.016);
        assert!(!z.just_calm);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=3
        z.tick(4.0); // 3*4 = 12
        assert!((z.alert - 12.0).abs() < 1e-3);
    }

    // --- is_alarmed / is_calm ---

    #[test]
    fn is_alarmed_false_when_disabled() {
        let mut z = z();
        z.alert = 100.0;
        z.enabled = false;
        assert!(!z.is_alarmed());
    }

    #[test]
    fn is_calm_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_calm());
    }

    // --- alert_fraction / effective_vigilance ---

    #[test]
    fn alert_fraction_zero_when_calm() {
        assert_eq!(z().alert_fraction(), 0.0);
    }

    #[test]
    fn alert_fraction_half_at_midpoint() {
        let mut z = z();
        z.alert = 50.0;
        assert!((z.alert_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_vigilance_zero_when_calm() {
        assert_eq!(z().effective_vigilance(100.0), 0.0);
    }

    #[test]
    fn effective_vigilance_scales_with_alert() {
        let mut z = z();
        z.alert = 80.0;
        assert!((z.effective_vigilance(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_vigilance_zero_when_disabled() {
        let mut z = z();
        z.alert = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_vigilance(100.0), 0.0);
    }
}
