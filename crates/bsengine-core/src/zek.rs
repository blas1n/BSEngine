use bevy_ecs::prelude::Component;

/// Forced-labor quota tracker. `output` builds via `labor(amount)` and
/// accumulates passively at `toil_rate` per second in `tick(dt)` or
/// is confiscated via `confiscate(amount)`.
///
/// Models Soviet-penal-colony work-quota fill levels, gulag-prisoner labor-
/// output accumulation bars, forced-labor productivity saturation gauges,
/// penal-brigade task-completion trackers, political-prisoner work-norm
/// meters, labor-camp output-quota fill levels, corrective-labor-colony
/// productivity indicators, brigade-competition output-saturation bars,
/// or any mechanic where relentless extraction of toil from an unwilling
/// workforce fills a quota until the commissar arrives to confiscate the
/// accumulated output and reset the clock for the next brutal shift.
///
/// `labor(amount)` adds output; fires `just_fulfilled` when first
/// reaching `max_output`. No-op when disabled.
///
/// `confiscate(amount)` reduces output immediately; fires `just_exhausted`
/// when reaching 0. No-op when disabled or already exhausted.
///
/// `tick(dt)` clears both flags, then increases output by
/// `toil_rate * dt` (capped at `max_output`). Fires `just_fulfilled`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_fulfilled()` returns `output >= max_output && enabled`.
///
/// `is_exhausted()` returns `output == 0.0` (not gated by `enabled`).
///
/// `output_fraction()` returns `(output / max_output).clamp(0, 1)`.
///
/// `effective_productivity(scale)` returns `scale * output_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — toils at 2 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zek {
    pub output: f32,
    pub max_output: f32,
    pub toil_rate: f32,
    pub just_fulfilled: bool,
    pub just_exhausted: bool,
    pub enabled: bool,
}

impl Zek {
    pub fn new(max_output: f32, toil_rate: f32) -> Self {
        Self {
            output: 0.0,
            max_output: max_output.max(0.1),
            toil_rate: toil_rate.max(0.0),
            just_fulfilled: false,
            just_exhausted: false,
            enabled: true,
        }
    }

    /// Add output; fires `just_fulfilled` when first reaching max.
    /// No-op when disabled.
    pub fn labor(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.output < self.max_output;
        self.output = (self.output + amount).min(self.max_output);
        if was_below && self.output >= self.max_output {
            self.just_fulfilled = true;
        }
    }

    /// Reduce output; fires `just_exhausted` when reaching 0.
    /// No-op when disabled or already exhausted.
    pub fn confiscate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.output <= 0.0 {
            return;
        }
        self.output = (self.output - amount).max(0.0);
        if self.output <= 0.0 {
            self.just_exhausted = true;
        }
    }

    /// Clear flags, then increase output by `toil_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_fulfilled = false;
        self.just_exhausted = false;
        if self.enabled && self.toil_rate > 0.0 && self.output < self.max_output {
            let was_below = self.output < self.max_output;
            self.output = (self.output + self.toil_rate * dt).min(self.max_output);
            if was_below && self.output >= self.max_output {
                self.just_fulfilled = true;
            }
        }
    }

    /// `true` when output is at maximum and component is enabled.
    pub fn is_fulfilled(&self) -> bool {
        self.output >= self.max_output && self.enabled
    }

    /// `true` when output is 0 (not gated by `enabled`).
    pub fn is_exhausted(&self) -> bool {
        self.output == 0.0
    }

    /// Fraction of maximum output [0.0, 1.0].
    pub fn output_fraction(&self) -> f32 {
        (self.output / self.max_output).clamp(0.0, 1.0)
    }

    /// Returns `scale * output_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_productivity(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.output_fraction()
    }
}

impl Default for Zek {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zek {
        Zek::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_exhausted() {
        let z = z();
        assert_eq!(z.output, 0.0);
        assert!(z.is_exhausted());
        assert!(!z.is_fulfilled());
    }

    #[test]
    fn new_clamps_max_output() {
        let z = Zek::new(-5.0, 2.0);
        assert!((z.max_output - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_toil_rate() {
        let z = Zek::new(100.0, -2.0);
        assert_eq!(z.toil_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zek::default();
        assert!((z.max_output - 100.0).abs() < 1e-5);
        assert!((z.toil_rate - 2.0).abs() < 1e-5);
    }

    // --- labor ---

    #[test]
    fn labor_adds_output() {
        let mut z = z();
        z.labor(40.0);
        assert!((z.output - 40.0).abs() < 1e-3);
    }

    #[test]
    fn labor_clamps_at_max() {
        let mut z = z();
        z.labor(200.0);
        assert!((z.output - 100.0).abs() < 1e-3);
    }

    #[test]
    fn labor_fires_just_fulfilled_at_max() {
        let mut z = z();
        z.labor(100.0);
        assert!(z.just_fulfilled);
        assert!(z.is_fulfilled());
    }

    #[test]
    fn labor_no_just_fulfilled_when_already_at_max() {
        let mut z = z();
        z.output = 100.0;
        z.labor(10.0);
        assert!(!z.just_fulfilled);
    }

    #[test]
    fn labor_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.labor(50.0);
        assert_eq!(z.output, 0.0);
    }

    #[test]
    fn labor_no_op_when_amount_zero() {
        let mut z = z();
        z.labor(0.0);
        assert_eq!(z.output, 0.0);
    }

    // --- confiscate ---

    #[test]
    fn confiscate_reduces_output() {
        let mut z = z();
        z.output = 60.0;
        z.confiscate(20.0);
        assert!((z.output - 40.0).abs() < 1e-3);
    }

    #[test]
    fn confiscate_clamps_at_zero() {
        let mut z = z();
        z.output = 30.0;
        z.confiscate(200.0);
        assert_eq!(z.output, 0.0);
    }

    #[test]
    fn confiscate_fires_just_exhausted_at_zero() {
        let mut z = z();
        z.output = 30.0;
        z.confiscate(30.0);
        assert!(z.just_exhausted);
    }

    #[test]
    fn confiscate_no_op_when_already_exhausted() {
        let mut z = z();
        z.confiscate(10.0);
        assert!(!z.just_exhausted);
    }

    #[test]
    fn confiscate_no_op_when_disabled() {
        let mut z = z();
        z.output = 50.0;
        z.enabled = false;
        z.confiscate(50.0);
        assert!((z.output - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_toils_output() {
        let mut z = z(); // rate=2
        z.tick(3.0); // 0 + 2*3 = 6
        assert!((z.output - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_fulfilled_on_toil_to_max() {
        let mut z = Zek::new(100.0, 200.0);
        z.output = 95.0;
        z.tick(1.0);
        assert!(z.just_fulfilled);
        assert!(z.is_fulfilled());
    }

    #[test]
    fn tick_no_toil_when_already_fulfilled() {
        let mut z = z();
        z.output = 100.0;
        z.tick(1.0);
        assert!(!z.just_fulfilled);
    }

    #[test]
    fn tick_no_toil_when_rate_zero() {
        let mut z = Zek::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.output, 0.0);
    }

    #[test]
    fn tick_no_toil_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.output, 0.0);
    }

    #[test]
    fn tick_clears_just_fulfilled() {
        let mut z = Zek::new(100.0, 200.0);
        z.output = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_fulfilled);
    }

    #[test]
    fn tick_clears_just_exhausted() {
        let mut z = z();
        z.output = 10.0;
        z.confiscate(10.0);
        z.tick(0.016);
        assert!(!z.just_exhausted);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2
        z.tick(5.0); // 2*5 = 10
        assert!((z.output - 10.0).abs() < 1e-3);
    }

    // --- is_fulfilled / is_exhausted ---

    #[test]
    fn is_fulfilled_false_when_disabled() {
        let mut z = z();
        z.output = 100.0;
        z.enabled = false;
        assert!(!z.is_fulfilled());
    }

    #[test]
    fn is_exhausted_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_exhausted());
    }

    // --- output_fraction / effective_productivity ---

    #[test]
    fn output_fraction_zero_when_exhausted() {
        assert_eq!(z().output_fraction(), 0.0);
    }

    #[test]
    fn output_fraction_half_at_midpoint() {
        let mut z = z();
        z.output = 50.0;
        assert!((z.output_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_productivity_zero_when_exhausted() {
        assert_eq!(z().effective_productivity(100.0), 0.0);
    }

    #[test]
    fn effective_productivity_scales_with_output() {
        let mut z = z();
        z.output = 75.0;
        assert!((z.effective_productivity(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_productivity_zero_when_disabled() {
        let mut z = z();
        z.output = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_productivity(100.0), 0.0);
    }
}
