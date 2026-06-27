use bevy_ecs::prelude::Component;

/// Inquiry-progress tracker. `inquiry` builds via `investigate(amount)` and
/// advances passively at `probe_rate` per second in `tick(dt)` or is
/// abandoned immediately via `abandon(amount)`.
///
/// Models detective-investigation progress bars, philosophical-inquiry
/// depth gauges, scholarly-research saturation trackers, hypothesis-
/// testing progress indicators, evidence-accumulation fill levels,
/// epistemic-confidence meters, academic-pursuit advancement bars,
/// forensic-analysis coverage trackers, scientific-method progress
/// indicators, or any mechanic where methodical questioning pushes
/// steadily toward a conclusion that seems assured right up until a
/// contradictory fact surfaces and the inquiry must be backed out and
/// restarted from whatever embarrassingly obvious premise was overlooked.
///
/// `investigate(amount)` adds inquiry; fires `just_resolved` when first
/// reaching `max_inquiry`. No-op when disabled.
///
/// `abandon(amount)` reduces inquiry immediately; fires `just_abandoned`
/// when reaching 0. No-op when disabled or already abandoned.
///
/// `tick(dt)` clears both flags, then increases inquiry by
/// `probe_rate * dt` (capped at `max_inquiry`). Fires `just_resolved`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_resolved()` returns `inquiry >= max_inquiry && enabled`.
///
/// `is_abandoned()` returns `inquiry == 0.0` (not gated by `enabled`).
///
/// `inquiry_fraction()` returns `(inquiry / max_inquiry).clamp(0, 1)`.
///
/// `effective_insight(scale)` returns `scale * inquiry_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.5)` — probes at 2.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zetetic {
    pub inquiry: f32,
    pub max_inquiry: f32,
    pub probe_rate: f32,
    pub just_resolved: bool,
    pub just_abandoned: bool,
    pub enabled: bool,
}

impl Zetetic {
    pub fn new(max_inquiry: f32, probe_rate: f32) -> Self {
        Self {
            inquiry: 0.0,
            max_inquiry: max_inquiry.max(0.1),
            probe_rate: probe_rate.max(0.0),
            just_resolved: false,
            just_abandoned: false,
            enabled: true,
        }
    }

    /// Add inquiry; fires `just_resolved` when first reaching max.
    /// No-op when disabled.
    pub fn investigate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.inquiry < self.max_inquiry;
        self.inquiry = (self.inquiry + amount).min(self.max_inquiry);
        if was_below && self.inquiry >= self.max_inquiry {
            self.just_resolved = true;
        }
    }

    /// Reduce inquiry; fires `just_abandoned` when reaching 0.
    /// No-op when disabled or already abandoned.
    pub fn abandon(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.inquiry <= 0.0 {
            return;
        }
        self.inquiry = (self.inquiry - amount).max(0.0);
        if self.inquiry <= 0.0 {
            self.just_abandoned = true;
        }
    }

    /// Clear flags, then increase inquiry by `probe_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_resolved = false;
        self.just_abandoned = false;
        if self.enabled && self.probe_rate > 0.0 && self.inquiry < self.max_inquiry {
            let was_below = self.inquiry < self.max_inquiry;
            self.inquiry = (self.inquiry + self.probe_rate * dt).min(self.max_inquiry);
            if was_below && self.inquiry >= self.max_inquiry {
                self.just_resolved = true;
            }
        }
    }

    /// `true` when inquiry is at maximum and component is enabled.
    pub fn is_resolved(&self) -> bool {
        self.inquiry >= self.max_inquiry && self.enabled
    }

    /// `true` when inquiry is 0 (not gated by `enabled`).
    pub fn is_abandoned(&self) -> bool {
        self.inquiry == 0.0
    }

    /// Fraction of maximum inquiry [0.0, 1.0].
    pub fn inquiry_fraction(&self) -> f32 {
        (self.inquiry / self.max_inquiry).clamp(0.0, 1.0)
    }

    /// Returns `scale * inquiry_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_insight(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.inquiry_fraction()
    }
}

impl Default for Zetetic {
    fn default() -> Self {
        Self::new(100.0, 2.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zetetic {
        Zetetic::new(100.0, 2.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_abandoned() {
        let z = z();
        assert_eq!(z.inquiry, 0.0);
        assert!(z.is_abandoned());
        assert!(!z.is_resolved());
    }

    #[test]
    fn new_clamps_max_inquiry() {
        let z = Zetetic::new(-5.0, 2.5);
        assert!((z.max_inquiry - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_probe_rate() {
        let z = Zetetic::new(100.0, -2.5);
        assert_eq!(z.probe_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zetetic::default();
        assert!((z.max_inquiry - 100.0).abs() < 1e-5);
        assert!((z.probe_rate - 2.5).abs() < 1e-5);
    }

    // --- investigate ---

    #[test]
    fn investigate_adds_inquiry() {
        let mut z = z();
        z.investigate(40.0);
        assert!((z.inquiry - 40.0).abs() < 1e-3);
    }

    #[test]
    fn investigate_clamps_at_max() {
        let mut z = z();
        z.investigate(200.0);
        assert!((z.inquiry - 100.0).abs() < 1e-3);
    }

    #[test]
    fn investigate_fires_just_resolved_at_max() {
        let mut z = z();
        z.investigate(100.0);
        assert!(z.just_resolved);
        assert!(z.is_resolved());
    }

    #[test]
    fn investigate_no_just_resolved_when_already_at_max() {
        let mut z = z();
        z.inquiry = 100.0;
        z.investigate(10.0);
        assert!(!z.just_resolved);
    }

    #[test]
    fn investigate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.investigate(50.0);
        assert_eq!(z.inquiry, 0.0);
    }

    #[test]
    fn investigate_no_op_when_amount_zero() {
        let mut z = z();
        z.investigate(0.0);
        assert_eq!(z.inquiry, 0.0);
    }

    // --- abandon ---

    #[test]
    fn abandon_reduces_inquiry() {
        let mut z = z();
        z.inquiry = 60.0;
        z.abandon(20.0);
        assert!((z.inquiry - 40.0).abs() < 1e-3);
    }

    #[test]
    fn abandon_clamps_at_zero() {
        let mut z = z();
        z.inquiry = 30.0;
        z.abandon(200.0);
        assert_eq!(z.inquiry, 0.0);
    }

    #[test]
    fn abandon_fires_just_abandoned_at_zero() {
        let mut z = z();
        z.inquiry = 30.0;
        z.abandon(30.0);
        assert!(z.just_abandoned);
    }

    #[test]
    fn abandon_no_op_when_already_abandoned() {
        let mut z = z();
        z.abandon(10.0);
        assert!(!z.just_abandoned);
    }

    #[test]
    fn abandon_no_op_when_disabled() {
        let mut z = z();
        z.inquiry = 50.0;
        z.enabled = false;
        z.abandon(50.0);
        assert!((z.inquiry - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_probes_inquiry() {
        let mut z = z(); // rate=2.5
        z.tick(2.0); // 0 + 2.5*2 = 5
        assert!((z.inquiry - 5.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_resolved_on_probe_to_max() {
        let mut z = Zetetic::new(100.0, 200.0);
        z.inquiry = 95.0;
        z.tick(1.0);
        assert!(z.just_resolved);
        assert!(z.is_resolved());
    }

    #[test]
    fn tick_no_probe_when_already_resolved() {
        let mut z = z();
        z.inquiry = 100.0;
        z.tick(1.0);
        assert!(!z.just_resolved);
    }

    #[test]
    fn tick_no_probe_when_rate_zero() {
        let mut z = Zetetic::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.inquiry, 0.0);
    }

    #[test]
    fn tick_no_probe_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.inquiry, 0.0);
    }

    #[test]
    fn tick_clears_just_resolved() {
        let mut z = Zetetic::new(100.0, 200.0);
        z.inquiry = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_resolved);
    }

    #[test]
    fn tick_clears_just_abandoned() {
        let mut z = z();
        z.inquiry = 10.0;
        z.abandon(10.0);
        z.tick(0.016);
        assert!(!z.just_abandoned);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2.5
        z.tick(4.0); // 2.5*4 = 10
        assert!((z.inquiry - 10.0).abs() < 1e-3);
    }

    // --- is_resolved / is_abandoned ---

    #[test]
    fn is_resolved_false_when_disabled() {
        let mut z = z();
        z.inquiry = 100.0;
        z.enabled = false;
        assert!(!z.is_resolved());
    }

    #[test]
    fn is_abandoned_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_abandoned());
    }

    // --- inquiry_fraction / effective_insight ---

    #[test]
    fn inquiry_fraction_zero_when_abandoned() {
        assert_eq!(z().inquiry_fraction(), 0.0);
    }

    #[test]
    fn inquiry_fraction_half_at_midpoint() {
        let mut z = z();
        z.inquiry = 50.0;
        assert!((z.inquiry_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_insight_zero_when_abandoned() {
        assert_eq!(z().effective_insight(100.0), 0.0);
    }

    #[test]
    fn effective_insight_scales_with_inquiry() {
        let mut z = z();
        z.inquiry = 75.0;
        assert!((z.effective_insight(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_insight_zero_when_disabled() {
        let mut z = z();
        z.inquiry = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_insight(100.0), 0.0);
    }
}
