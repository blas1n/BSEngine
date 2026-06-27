use bevy_ecs::prelude::Component;

/// Knockout-stun accumulator. `daze` builds via `bonk(amount)` and
/// rises passively at `bonk_rate` per second in `tick(dt)` or
/// clears immediately via `recover(amount)`.
///
/// Models cartoon-knockout meters, stun-accumulation bars,
/// head-knock dazing trackers, comic-impact force accumulators,
/// sleep-onset fatigue fill levels, party-game bonk hit counters,
/// comedy-concussion build-up indicators, anvil-drop daze gauges,
/// whack-a-mole stunner trackers, or any mechanic where repeated
/// bonks accumulate into a full knockout state.
///
/// `bonk(amount)` adds daze; fires `just_knocked_out` when first
/// reaching `max_daze`. No-op when disabled.
///
/// `recover(amount)` reduces daze immediately; fires `just_cleared`
/// when reaching 0. No-op when disabled or already cleared.
///
/// `tick(dt)` clears both flags, then increases daze by
/// `bonk_rate * dt` (capped at `max_daze`). Fires `just_knocked_out`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_knocked_out()` returns `daze >= max_daze && enabled`.
///
/// `is_cleared()` returns `daze == 0.0` (not gated by `enabled`).
///
/// `daze_fraction()` returns `(daze / max_daze).clamp(0, 1)`.
///
/// `effective_stun(scale)` returns `scale * daze_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — bonks passively at 2 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zonk {
    pub daze: f32,
    pub max_daze: f32,
    pub bonk_rate: f32,
    pub just_knocked_out: bool,
    pub just_cleared: bool,
    pub enabled: bool,
}

impl Zonk {
    pub fn new(max_daze: f32, bonk_rate: f32) -> Self {
        Self {
            daze: 0.0,
            max_daze: max_daze.max(0.1),
            bonk_rate: bonk_rate.max(0.0),
            just_knocked_out: false,
            just_cleared: false,
            enabled: true,
        }
    }

    /// Add daze; fires `just_knocked_out` when first reaching max.
    /// No-op when disabled.
    pub fn bonk(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.daze < self.max_daze;
        self.daze = (self.daze + amount).min(self.max_daze);
        if was_below && self.daze >= self.max_daze {
            self.just_knocked_out = true;
        }
    }

    /// Reduce daze; fires `just_cleared` when reaching 0.
    /// No-op when disabled or already cleared.
    pub fn recover(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.daze <= 0.0 {
            return;
        }
        self.daze = (self.daze - amount).max(0.0);
        if self.daze <= 0.0 {
            self.just_cleared = true;
        }
    }

    /// Clear flags, then increase daze by `bonk_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_knocked_out = false;
        self.just_cleared = false;
        if self.enabled && self.bonk_rate > 0.0 && self.daze < self.max_daze {
            let was_below = self.daze < self.max_daze;
            self.daze = (self.daze + self.bonk_rate * dt).min(self.max_daze);
            if was_below && self.daze >= self.max_daze {
                self.just_knocked_out = true;
            }
        }
    }

    /// `true` when daze is at maximum and component is enabled.
    pub fn is_knocked_out(&self) -> bool {
        self.daze >= self.max_daze && self.enabled
    }

    /// `true` when daze is 0 (not gated by `enabled`).
    pub fn is_cleared(&self) -> bool {
        self.daze == 0.0
    }

    /// Fraction of maximum daze [0.0, 1.0].
    pub fn daze_fraction(&self) -> f32 {
        (self.daze / self.max_daze).clamp(0.0, 1.0)
    }

    /// Returns `scale * daze_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_stun(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.daze_fraction()
    }
}

impl Default for Zonk {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zonk {
        Zonk::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_cleared() {
        let z = z();
        assert_eq!(z.daze, 0.0);
        assert!(z.is_cleared());
        assert!(!z.is_knocked_out());
    }

    #[test]
    fn new_clamps_max_daze() {
        let z = Zonk::new(-5.0, 2.0);
        assert!((z.max_daze - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_bonk_rate() {
        let z = Zonk::new(100.0, -3.0);
        assert_eq!(z.bonk_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zonk::default();
        assert!((z.max_daze - 100.0).abs() < 1e-5);
        assert!((z.bonk_rate - 2.0).abs() < 1e-5);
    }

    // --- bonk ---

    #[test]
    fn bonk_adds_daze() {
        let mut z = z();
        z.bonk(40.0);
        assert!((z.daze - 40.0).abs() < 1e-3);
    }

    #[test]
    fn bonk_clamps_at_max() {
        let mut z = z();
        z.bonk(200.0);
        assert!((z.daze - 100.0).abs() < 1e-3);
    }

    #[test]
    fn bonk_fires_just_knocked_out_at_max() {
        let mut z = z();
        z.bonk(100.0);
        assert!(z.just_knocked_out);
        assert!(z.is_knocked_out());
    }

    #[test]
    fn bonk_no_just_knocked_out_when_already_at_max() {
        let mut z = z();
        z.daze = 100.0;
        z.bonk(10.0);
        assert!(!z.just_knocked_out);
    }

    #[test]
    fn bonk_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.bonk(50.0);
        assert_eq!(z.daze, 0.0);
    }

    #[test]
    fn bonk_no_op_when_amount_zero() {
        let mut z = z();
        z.bonk(0.0);
        assert_eq!(z.daze, 0.0);
    }

    // --- recover ---

    #[test]
    fn recover_reduces_daze() {
        let mut z = z();
        z.daze = 60.0;
        z.recover(20.0);
        assert!((z.daze - 40.0).abs() < 1e-3);
    }

    #[test]
    fn recover_clamps_at_zero() {
        let mut z = z();
        z.daze = 30.0;
        z.recover(200.0);
        assert_eq!(z.daze, 0.0);
    }

    #[test]
    fn recover_fires_just_cleared_at_zero() {
        let mut z = z();
        z.daze = 30.0;
        z.recover(30.0);
        assert!(z.just_cleared);
    }

    #[test]
    fn recover_no_op_when_already_cleared() {
        let mut z = z();
        z.recover(10.0);
        assert!(!z.just_cleared);
    }

    #[test]
    fn recover_no_op_when_disabled() {
        let mut z = z();
        z.daze = 50.0;
        z.enabled = false;
        z.recover(50.0);
        assert!((z.daze - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_increases_daze() {
        let mut z = z(); // rate=2
        z.tick(1.0); // 0 + 2 = 2
        assert!((z.daze - 2.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_knocked_out_on_bonk_to_max() {
        let mut z = Zonk::new(100.0, 200.0);
        z.daze = 95.0;
        z.tick(1.0);
        assert!(z.just_knocked_out);
        assert!(z.is_knocked_out());
    }

    #[test]
    fn tick_no_bonk_when_already_knocked_out() {
        let mut z = z();
        z.daze = 100.0;
        z.tick(1.0);
        assert!(!z.just_knocked_out);
    }

    #[test]
    fn tick_no_bonk_when_rate_zero() {
        let mut z = Zonk::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.daze, 0.0);
    }

    #[test]
    fn tick_no_bonk_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.daze, 0.0);
    }

    #[test]
    fn tick_clears_just_knocked_out() {
        let mut z = Zonk::new(100.0, 200.0);
        z.daze = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_knocked_out);
    }

    #[test]
    fn tick_clears_just_cleared() {
        let mut z = z();
        z.daze = 10.0;
        z.recover(10.0);
        z.tick(0.016);
        assert!(!z.just_cleared);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2
        z.tick(5.0); // 2*5 = 10
        assert!((z.daze - 10.0).abs() < 1e-3);
    }

    // --- is_knocked_out / is_cleared ---

    #[test]
    fn is_knocked_out_false_when_disabled() {
        let mut z = z();
        z.daze = 100.0;
        z.enabled = false;
        assert!(!z.is_knocked_out());
    }

    #[test]
    fn is_cleared_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_cleared());
    }

    // --- daze_fraction / effective_stun ---

    #[test]
    fn daze_fraction_zero_when_cleared() {
        assert_eq!(z().daze_fraction(), 0.0);
    }

    #[test]
    fn daze_fraction_half_at_midpoint() {
        let mut z = z();
        z.daze = 50.0;
        assert!((z.daze_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_stun_zero_when_cleared() {
        assert_eq!(z().effective_stun(100.0), 0.0);
    }

    #[test]
    fn effective_stun_scales_with_daze() {
        let mut z = z();
        z.daze = 80.0;
        assert!((z.effective_stun(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_stun_zero_when_disabled() {
        let mut z = z();
        z.daze = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_stun(100.0), 0.0);
    }
}
