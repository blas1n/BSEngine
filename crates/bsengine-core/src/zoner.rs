use bevy_ecs::prelude::Component;

/// Zone-control tracker. `control` builds via `claim(amount)` and
/// expands passively at `expand_rate` per second in `tick(dt)` or is
/// surrendered immediately via `cede(amount)`.
///
/// Models territory-control meters, area-denial charge gauges,
/// zone-influence fill levels, checkpoint-capture progress bars,
/// resource-node dominance trackers, control-point ownership meters,
/// spatial-influence accumulators, or any mechanic where an entity
/// asserts dominance over an area that rivals can contest.
///
/// `claim(amount)` adds control; fires `just_dominant` when first
/// reaching `max_control`. No-op when disabled.
///
/// `cede(amount)` reduces control immediately; fires `just_yielded`
/// when reaching 0. No-op when disabled or already yielded.
///
/// `tick(dt)` clears both flags, then increases control by
/// `expand_rate * dt` (capped at `max_control`). Fires `just_dominant`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_dominant()` returns `control >= max_control && enabled`.
///
/// `is_yielded()` returns `control == 0.0` (not gated by `enabled`).
///
/// `control_fraction()` returns `(control / max_control).clamp(0, 1)`.
///
/// `effective_influence(scale)` returns `scale * control_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 5.0)` — expands control at 5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoner {
    pub control: f32,
    pub max_control: f32,
    pub expand_rate: f32,
    pub just_dominant: bool,
    pub just_yielded: bool,
    pub enabled: bool,
}

impl Zoner {
    pub fn new(max_control: f32, expand_rate: f32) -> Self {
        Self {
            control: 0.0,
            max_control: max_control.max(0.1),
            expand_rate: expand_rate.max(0.0),
            just_dominant: false,
            just_yielded: false,
            enabled: true,
        }
    }

    /// Add control; fires `just_dominant` when first reaching max.
    /// No-op when disabled.
    pub fn claim(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.control < self.max_control;
        self.control = (self.control + amount).min(self.max_control);
        if was_below && self.control >= self.max_control {
            self.just_dominant = true;
        }
    }

    /// Reduce control; fires `just_yielded` when reaching 0.
    /// No-op when disabled or already yielded.
    pub fn cede(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.control <= 0.0 {
            return;
        }
        self.control = (self.control - amount).max(0.0);
        if self.control <= 0.0 {
            self.just_yielded = true;
        }
    }

    /// Clear flags, then increase control by `expand_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_dominant = false;
        self.just_yielded = false;
        if self.enabled && self.expand_rate > 0.0 && self.control < self.max_control {
            let was_below = self.control < self.max_control;
            self.control = (self.control + self.expand_rate * dt).min(self.max_control);
            if was_below && self.control >= self.max_control {
                self.just_dominant = true;
            }
        }
    }

    /// `true` when control is at maximum and component is enabled.
    pub fn is_dominant(&self) -> bool {
        self.control >= self.max_control && self.enabled
    }

    /// `true` when control is 0 (not gated by `enabled`).
    pub fn is_yielded(&self) -> bool {
        self.control == 0.0
    }

    /// Fraction of maximum control [0.0, 1.0].
    pub fn control_fraction(&self) -> f32 {
        (self.control / self.max_control).clamp(0.0, 1.0)
    }

    /// Returns `scale * control_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_influence(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.control_fraction()
    }
}

impl Default for Zoner {
    fn default() -> Self {
        Self::new(100.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoner {
        Zoner::new(100.0, 5.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_yielded() {
        let z = z();
        assert_eq!(z.control, 0.0);
        assert!(z.is_yielded());
        assert!(!z.is_dominant());
    }

    #[test]
    fn new_clamps_max_control() {
        let z = Zoner::new(-5.0, 5.0);
        assert!((z.max_control - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_expand_rate() {
        let z = Zoner::new(100.0, -3.0);
        assert_eq!(z.expand_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoner::default();
        assert!((z.max_control - 100.0).abs() < 1e-5);
        assert!((z.expand_rate - 5.0).abs() < 1e-5);
    }

    // --- claim ---

    #[test]
    fn claim_adds_control() {
        let mut z = z();
        z.claim(40.0);
        assert!((z.control - 40.0).abs() < 1e-3);
    }

    #[test]
    fn claim_clamps_at_max() {
        let mut z = z();
        z.claim(200.0);
        assert!((z.control - 100.0).abs() < 1e-3);
    }

    #[test]
    fn claim_fires_just_dominant_at_max() {
        let mut z = z();
        z.claim(100.0);
        assert!(z.just_dominant);
        assert!(z.is_dominant());
    }

    #[test]
    fn claim_no_just_dominant_when_already_at_max() {
        let mut z = z();
        z.control = 100.0;
        z.claim(10.0);
        assert!(!z.just_dominant);
    }

    #[test]
    fn claim_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.claim(50.0);
        assert_eq!(z.control, 0.0);
    }

    #[test]
    fn claim_no_op_when_amount_zero() {
        let mut z = z();
        z.claim(0.0);
        assert_eq!(z.control, 0.0);
    }

    // --- cede ---

    #[test]
    fn cede_reduces_control() {
        let mut z = z();
        z.control = 60.0;
        z.cede(20.0);
        assert!((z.control - 40.0).abs() < 1e-3);
    }

    #[test]
    fn cede_clamps_at_zero() {
        let mut z = z();
        z.control = 30.0;
        z.cede(200.0);
        assert_eq!(z.control, 0.0);
    }

    #[test]
    fn cede_fires_just_yielded_at_zero() {
        let mut z = z();
        z.control = 30.0;
        z.cede(30.0);
        assert!(z.just_yielded);
    }

    #[test]
    fn cede_no_op_when_already_yielded() {
        let mut z = z();
        z.cede(10.0);
        assert!(!z.just_yielded);
    }

    #[test]
    fn cede_no_op_when_disabled() {
        let mut z = z();
        z.control = 50.0;
        z.enabled = false;
        z.cede(50.0);
        assert!((z.control - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_expands_control() {
        let mut z = z(); // rate=5
        z.tick(1.0); // 0 + 5 = 5
        assert!((z.control - 5.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_dominant_on_expand_to_max() {
        let mut z = Zoner::new(100.0, 200.0);
        z.control = 95.0;
        z.tick(1.0);
        assert!(z.just_dominant);
        assert!(z.is_dominant());
    }

    #[test]
    fn tick_no_expand_when_already_dominant() {
        let mut z = z();
        z.control = 100.0;
        z.tick(1.0);
        assert!(!z.just_dominant);
    }

    #[test]
    fn tick_no_expand_when_rate_zero() {
        let mut z = Zoner::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.control, 0.0);
    }

    #[test]
    fn tick_no_expand_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.control, 0.0);
    }

    #[test]
    fn tick_clears_just_dominant() {
        let mut z = Zoner::new(100.0, 200.0);
        z.control = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_dominant);
    }

    #[test]
    fn tick_clears_just_yielded() {
        let mut z = z();
        z.control = 10.0;
        z.cede(10.0);
        z.tick(0.016);
        assert!(!z.just_yielded);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=5
        z.tick(3.0); // 5*3 = 15
        assert!((z.control - 15.0).abs() < 1e-3);
    }

    // --- is_dominant / is_yielded ---

    #[test]
    fn is_dominant_false_when_disabled() {
        let mut z = z();
        z.control = 100.0;
        z.enabled = false;
        assert!(!z.is_dominant());
    }

    #[test]
    fn is_yielded_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_yielded());
    }

    // --- control_fraction / effective_influence ---

    #[test]
    fn control_fraction_zero_when_yielded() {
        assert_eq!(z().control_fraction(), 0.0);
    }

    #[test]
    fn control_fraction_half_at_midpoint() {
        let mut z = z();
        z.control = 50.0;
        assert!((z.control_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_influence_zero_when_yielded() {
        assert_eq!(z().effective_influence(100.0), 0.0);
    }

    #[test]
    fn effective_influence_scales_with_control() {
        let mut z = z();
        z.control = 65.0;
        assert!((z.effective_influence(100.0) - 65.0).abs() < 1e-3);
    }

    #[test]
    fn effective_influence_zero_when_disabled() {
        let mut z = z();
        z.control = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_influence(100.0), 0.0);
    }
}
