use bevy_ecs::prelude::Component;

/// HP-threshold warning with cooldown: when the entity's health fraction
/// drops below `hp_threshold`, the health system calls `check(hp_fraction)`,
/// which fires `just_triggered` and resets the `cooldown_timer`. Repeated
/// `check` calls are no-ops while the timer is still counting down (rate
/// limiting prevents event spam on rapid damage). `tick(dt)` counts down the
/// timer and clears one-frame flags.
///
/// The warning system (ally AI, UI overlay, audio) observes `just_triggered`
/// each frame to react to the alarm. `is_warning()` returns `true` while the
/// cooldown has not yet elapsed — useful for persistent visual indicators.
///
/// `check(hp_fraction)` is a no-op when disabled or the cooldown timer is
/// still running.
///
/// Distinct from `Alarm` (breach detection on spatial intrusion), `Notice`
/// (enemy detection radius), and `Beacon` (continuous AoE signal): Warn is a
/// **HP-threshold cooldown warning** — it fires a one-frame event the first
/// time health crosses a critical level, rate-limited to avoid flooding the
/// event bus on sustained damage.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Warn {
    /// HP fraction [0.0, 1.0] below which the warning fires.
    pub hp_threshold: f32,
    /// Minimum seconds between consecutive warning events. Clamped ≥ 0.0.
    pub cooldown: f32,
    /// Remaining cooldown seconds. Counts down to 0 after a warning fires.
    pub cooldown_timer: f32,
    pub just_triggered: bool,
    pub enabled: bool,
}

impl Warn {
    pub fn new(hp_threshold: f32, cooldown: f32) -> Self {
        Self {
            hp_threshold: hp_threshold.clamp(0.0, 1.0),
            cooldown: cooldown.max(0.0),
            cooldown_timer: 0.0,
            just_triggered: false,
            enabled: true,
        }
    }

    /// Evaluate the current HP fraction and fire the warning if eligible.
    /// Fires when `hp_fraction < hp_threshold` AND `cooldown_timer <= 0`
    /// AND the component is enabled. Sets `just_triggered` and resets
    /// `cooldown_timer` to `cooldown`. No-op otherwise.
    pub fn check(&mut self, hp_fraction: f32) {
        if !self.enabled || self.cooldown_timer > 0.0 {
            return;
        }
        if hp_fraction < self.hp_threshold {
            self.just_triggered = true;
            self.cooldown_timer = self.cooldown;
        }
    }

    /// Advance the cooldown timer by `dt` seconds (floored at 0.0). Clears
    /// one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_triggered = false;

        if self.cooldown_timer > 0.0 {
            self.cooldown_timer -= dt;
            if self.cooldown_timer < 0.0 {
                self.cooldown_timer = 0.0;
            }
        }
    }

    /// `true` while the cooldown is still counting down (warning recently fired).
    pub fn is_warning(&self) -> bool {
        self.cooldown_timer > 0.0
    }

    /// `true` when the component is ready to fire a new warning.
    pub fn is_ready(&self) -> bool {
        self.enabled && self.cooldown_timer <= 0.0
    }
}

impl Default for Warn {
    fn default() -> Self {
        Self::new(0.25, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_fires_when_below_threshold() {
        let mut w = Warn::new(0.3, 5.0);
        w.check(0.2);
        assert!(w.just_triggered);
        assert!(w.is_warning());
    }

    #[test]
    fn check_no_op_when_above_threshold() {
        let mut w = Warn::new(0.3, 5.0);
        w.check(0.5);
        assert!(!w.just_triggered);
        assert!(!w.is_warning());
    }

    #[test]
    fn check_no_op_when_equal_to_threshold() {
        let mut w = Warn::new(0.3, 5.0);
        w.check(0.3); // not strictly below
        assert!(!w.just_triggered);
    }

    #[test]
    fn check_no_op_while_cooldown_running() {
        let mut w = Warn::new(0.3, 5.0);
        w.check(0.1); // fires
        w.tick(0.016);
        w.check(0.1); // still cooling down
        assert!(!w.just_triggered);
    }

    #[test]
    fn check_no_op_when_disabled() {
        let mut w = Warn::new(0.3, 5.0);
        w.enabled = false;
        w.check(0.1);
        assert!(!w.just_triggered);
    }

    #[test]
    fn check_resets_cooldown_timer() {
        let mut w = Warn::new(0.3, 5.0);
        w.check(0.1);
        assert!((w.cooldown_timer - 5.0).abs() < 1e-5);
    }

    #[test]
    fn tick_counts_down_cooldown() {
        let mut w = Warn::new(0.3, 5.0);
        w.check(0.1);
        w.tick(2.0);
        assert!((w.cooldown_timer - 3.0).abs() < 1e-3);
    }

    #[test]
    fn tick_floors_timer_at_zero() {
        let mut w = Warn::new(0.3, 2.0);
        w.check(0.1);
        w.tick(5.0);
        assert_eq!(w.cooldown_timer, 0.0);
        assert!(!w.is_warning());
    }

    #[test]
    fn tick_clears_just_triggered() {
        let mut w = Warn::new(0.3, 5.0);
        w.check(0.1);
        w.tick(0.016);
        assert!(!w.just_triggered);
    }

    #[test]
    fn check_fires_again_after_cooldown() {
        let mut w = Warn::new(0.3, 2.0);
        w.check(0.1);
        w.tick(2.1);
        w.check(0.1);
        assert!(w.just_triggered);
    }

    #[test]
    fn is_warning_false_before_any_check() {
        let w = Warn::new(0.3, 5.0);
        assert!(!w.is_warning());
    }

    #[test]
    fn is_ready_true_initially() {
        let w = Warn::new(0.3, 5.0);
        assert!(w.is_ready());
    }

    #[test]
    fn is_ready_false_while_cooling_down() {
        let mut w = Warn::new(0.3, 5.0);
        w.check(0.1);
        assert!(!w.is_ready());
    }

    #[test]
    fn is_ready_false_when_disabled() {
        let mut w = Warn::new(0.3, 5.0);
        w.enabled = false;
        assert!(!w.is_ready());
    }

    #[test]
    fn zero_cooldown_fires_every_check() {
        let mut w = Warn::new(0.3, 0.0);
        w.check(0.1);
        assert!(w.just_triggered);
        w.tick(0.0);
        w.check(0.1);
        assert!(w.just_triggered);
    }

    #[test]
    fn hp_threshold_clamped_zero_to_one() {
        let w_low = Warn::new(-0.5, 5.0);
        let w_high = Warn::new(2.0, 5.0);
        assert!(w_low.hp_threshold >= 0.0);
        assert!(w_high.hp_threshold <= 1.0);
    }
}
