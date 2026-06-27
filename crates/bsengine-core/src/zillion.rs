use bevy_ecs::prelude::Component;

/// Hyperbolic-quantity accumulator. `count` builds via `accumulate(amount)`
/// and tallies passively at `tally_rate` per second in `tick(dt)` or is
/// spent immediately via `expend(amount)`.
///
/// Models score-multiplier fill levels, combo-counter saturation bars,
/// achievement-accumulation gauges, prestige-point tallying trackers,
/// kill-count escalation meters, resource-surplus overflow indicators,
/// high-score approach indicators, exponential-reward build-up bars,
/// crowd-counter fill levels, or any mechanic where a running total
/// climbs from single digits through hundreds and thousands and millions
/// until it reaches some frankly absurd astronomical figure that the
/// developer never actually expected anyone to hit but here we are
/// because a single idle player left it running for six weeks.
///
/// `accumulate(amount)` adds count; fires `just_astronomical` when first
/// reaching `max_count`. No-op when disabled.
///
/// `expend(amount)` reduces count immediately; fires `just_zeroed`
/// when reaching 0. No-op when disabled or already zeroed.
///
/// `tick(dt)` clears both flags, then increases count by
/// `tally_rate * dt` (capped at `max_count`). Fires `just_astronomical`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_astronomical()` returns `count >= max_count && enabled`.
///
/// `is_zeroed()` returns `count == 0.0` (not gated by `enabled`).
///
/// `count_fraction()` returns `(count / max_count).clamp(0, 1)`.
///
/// `effective_magnitude(scale)` returns `scale * count_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — tallies at 2 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zillion {
    pub count: f32,
    pub max_count: f32,
    pub tally_rate: f32,
    pub just_astronomical: bool,
    pub just_zeroed: bool,
    pub enabled: bool,
}

impl Zillion {
    pub fn new(max_count: f32, tally_rate: f32) -> Self {
        Self {
            count: 0.0,
            max_count: max_count.max(0.1),
            tally_rate: tally_rate.max(0.0),
            just_astronomical: false,
            just_zeroed: false,
            enabled: true,
        }
    }

    /// Add count; fires `just_astronomical` when first reaching max.
    /// No-op when disabled.
    pub fn accumulate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.count < self.max_count;
        self.count = (self.count + amount).min(self.max_count);
        if was_below && self.count >= self.max_count {
            self.just_astronomical = true;
        }
    }

    /// Reduce count; fires `just_zeroed` when reaching 0.
    /// No-op when disabled or already zeroed.
    pub fn expend(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.count <= 0.0 {
            return;
        }
        self.count = (self.count - amount).max(0.0);
        if self.count <= 0.0 {
            self.just_zeroed = true;
        }
    }

    /// Clear flags, then increase count by `tally_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_astronomical = false;
        self.just_zeroed = false;
        if self.enabled && self.tally_rate > 0.0 && self.count < self.max_count {
            let was_below = self.count < self.max_count;
            self.count = (self.count + self.tally_rate * dt).min(self.max_count);
            if was_below && self.count >= self.max_count {
                self.just_astronomical = true;
            }
        }
    }

    /// `true` when count is at maximum and component is enabled.
    pub fn is_astronomical(&self) -> bool {
        self.count >= self.max_count && self.enabled
    }

    /// `true` when count is 0 (not gated by `enabled`).
    pub fn is_zeroed(&self) -> bool {
        self.count == 0.0
    }

    /// Fraction of maximum count [0.0, 1.0].
    pub fn count_fraction(&self) -> f32 {
        (self.count / self.max_count).clamp(0.0, 1.0)
    }

    /// Returns `scale * count_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_magnitude(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.count_fraction()
    }
}

impl Default for Zillion {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zillion {
        Zillion::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_zeroed() {
        let z = z();
        assert_eq!(z.count, 0.0);
        assert!(z.is_zeroed());
        assert!(!z.is_astronomical());
    }

    #[test]
    fn new_clamps_max_count() {
        let z = Zillion::new(-5.0, 2.0);
        assert!((z.max_count - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_tally_rate() {
        let z = Zillion::new(100.0, -3.0);
        assert_eq!(z.tally_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zillion::default();
        assert!((z.max_count - 100.0).abs() < 1e-5);
        assert!((z.tally_rate - 2.0).abs() < 1e-5);
    }

    // --- accumulate ---

    #[test]
    fn accumulate_adds_count() {
        let mut z = z();
        z.accumulate(40.0);
        assert!((z.count - 40.0).abs() < 1e-3);
    }

    #[test]
    fn accumulate_clamps_at_max() {
        let mut z = z();
        z.accumulate(200.0);
        assert!((z.count - 100.0).abs() < 1e-3);
    }

    #[test]
    fn accumulate_fires_just_astronomical_at_max() {
        let mut z = z();
        z.accumulate(100.0);
        assert!(z.just_astronomical);
        assert!(z.is_astronomical());
    }

    #[test]
    fn accumulate_no_just_astronomical_when_already_at_max() {
        let mut z = z();
        z.count = 100.0;
        z.accumulate(10.0);
        assert!(!z.just_astronomical);
    }

    #[test]
    fn accumulate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.accumulate(50.0);
        assert_eq!(z.count, 0.0);
    }

    #[test]
    fn accumulate_no_op_when_amount_zero() {
        let mut z = z();
        z.accumulate(0.0);
        assert_eq!(z.count, 0.0);
    }

    // --- expend ---

    #[test]
    fn expend_reduces_count() {
        let mut z = z();
        z.count = 60.0;
        z.expend(20.0);
        assert!((z.count - 40.0).abs() < 1e-3);
    }

    #[test]
    fn expend_clamps_at_zero() {
        let mut z = z();
        z.count = 30.0;
        z.expend(200.0);
        assert_eq!(z.count, 0.0);
    }

    #[test]
    fn expend_fires_just_zeroed_at_zero() {
        let mut z = z();
        z.count = 30.0;
        z.expend(30.0);
        assert!(z.just_zeroed);
    }

    #[test]
    fn expend_no_op_when_already_zeroed() {
        let mut z = z();
        z.expend(10.0);
        assert!(!z.just_zeroed);
    }

    #[test]
    fn expend_no_op_when_disabled() {
        let mut z = z();
        z.count = 50.0;
        z.enabled = false;
        z.expend(50.0);
        assert!((z.count - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_tallies_count() {
        let mut z = z(); // rate=2
        z.tick(3.0); // 0 + 2*3 = 6
        assert!((z.count - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_astronomical_on_tally_to_max() {
        let mut z = Zillion::new(100.0, 200.0);
        z.count = 95.0;
        z.tick(1.0);
        assert!(z.just_astronomical);
        assert!(z.is_astronomical());
    }

    #[test]
    fn tick_no_tally_when_already_astronomical() {
        let mut z = z();
        z.count = 100.0;
        z.tick(1.0);
        assert!(!z.just_astronomical);
    }

    #[test]
    fn tick_no_tally_when_rate_zero() {
        let mut z = Zillion::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.count, 0.0);
    }

    #[test]
    fn tick_no_tally_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.count, 0.0);
    }

    #[test]
    fn tick_clears_just_astronomical() {
        let mut z = Zillion::new(100.0, 200.0);
        z.count = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_astronomical);
    }

    #[test]
    fn tick_clears_just_zeroed() {
        let mut z = z();
        z.count = 10.0;
        z.expend(10.0);
        z.tick(0.016);
        assert!(!z.just_zeroed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2
        z.tick(5.0); // 2*5 = 10
        assert!((z.count - 10.0).abs() < 1e-3);
    }

    // --- is_astronomical / is_zeroed ---

    #[test]
    fn is_astronomical_false_when_disabled() {
        let mut z = z();
        z.count = 100.0;
        z.enabled = false;
        assert!(!z.is_astronomical());
    }

    #[test]
    fn is_zeroed_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_zeroed());
    }

    // --- count_fraction / effective_magnitude ---

    #[test]
    fn count_fraction_zero_when_zeroed() {
        assert_eq!(z().count_fraction(), 0.0);
    }

    #[test]
    fn count_fraction_half_at_midpoint() {
        let mut z = z();
        z.count = 50.0;
        assert!((z.count_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_magnitude_zero_when_zeroed() {
        assert_eq!(z().effective_magnitude(100.0), 0.0);
    }

    #[test]
    fn effective_magnitude_scales_with_count() {
        let mut z = z();
        z.count = 75.0;
        assert!((z.effective_magnitude(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_magnitude_zero_when_disabled() {
        let mut z = z();
        z.count = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_magnitude(100.0), 0.0);
    }
}
