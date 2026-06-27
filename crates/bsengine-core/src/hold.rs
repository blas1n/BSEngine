use bevy_ecs::prelude::Component;

/// Charged-attack hold state: while the player holds the attack button,
/// `hold_time` accumulates toward `max_hold_time`. `power_fraction()` returns
/// `hold_time / max_hold_time` clamped to [0.0, 1.0], which the attack system
/// reads as a damage/range/velocity multiplier. `release()` returns the final
/// power fraction, fires `just_released`, and resets the state.
///
/// `start_hold()` begins holding and fires `just_began` on the idle → held
/// transition. `tick(dt)` advances `hold_time` (capped at `max_hold_time`) and
/// clears one-frame flags.
///
/// `start_hold()` is a no-op while already holding or when disabled.
/// `release()` returns 0.0 when not holding or disabled.
///
/// Distinct from `Charge` (a movement-based rush), `Windup` (a fixed delay
/// before an attack fires), and `Combo` (chaining multiple hits): Hold is the
/// **hold-button-for-power** mechanic — the longer the button is held, the
/// stronger the resulting attack.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Hold {
    /// Whether the entity is currently holding the attack button.
    pub active: bool,
    /// Elapsed hold time in seconds. Capped at `max_hold_time`.
    pub hold_time: f32,
    /// Seconds to reach full charge. Clamped ≥ 0.001.
    pub max_hold_time: f32,
    pub just_began: bool,
    pub just_released: bool,
    pub enabled: bool,
}

impl Hold {
    pub fn new(max_hold_time: f32) -> Self {
        Self {
            active: false,
            hold_time: 0.0,
            max_hold_time: max_hold_time.max(0.001),
            just_began: false,
            just_released: false,
            enabled: true,
        }
    }

    /// Begin holding. Fires `just_began` on the idle → active transition.
    /// No-op when already holding or disabled.
    pub fn start_hold(&mut self) {
        if !self.enabled || self.active {
            return;
        }
        self.active = true;
        self.just_began = true;
    }

    /// End the hold. Returns the power fraction [0.0, 1.0] at time of release,
    /// fires `just_released`, and resets `hold_time`. Returns 0.0 when not
    /// holding or disabled.
    pub fn release(&mut self) -> f32 {
        if !self.active || !self.enabled {
            return 0.0;
        }
        let power = self.power_fraction();
        self.active = false;
        self.hold_time = 0.0;
        self.just_released = true;
        power
    }

    /// Advance the hold timer by `dt` seconds (capped at `max_hold_time`).
    /// Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_began = false;
        self.just_released = false;

        if self.active {
            self.hold_time = (self.hold_time + dt).min(self.max_hold_time);
        }
    }

    /// Current charge fraction [0.0, 1.0]: `hold_time / max_hold_time`.
    pub fn power_fraction(&self) -> f32 {
        (self.hold_time / self.max_hold_time).clamp(0.0, 1.0)
    }

    /// `true` while the button is held and the component is enabled.
    pub fn is_holding(&self) -> bool {
        self.active && self.enabled
    }

    /// `true` when fully charged (`hold_time >= max_hold_time`).
    pub fn is_fully_charged(&self) -> bool {
        self.active && self.hold_time >= self.max_hold_time
    }
}

impl Default for Hold {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_hold_activates() {
        let mut h = Hold::new(1.0);
        h.start_hold();
        assert!(h.active);
        assert!(h.just_began);
        assert!(h.is_holding());
    }

    #[test]
    fn start_hold_no_op_when_already_active() {
        let mut h = Hold::new(1.0);
        h.start_hold();
        h.tick(0.016);
        h.start_hold(); // already active
        assert!(!h.just_began);
    }

    #[test]
    fn start_hold_no_op_when_disabled() {
        let mut h = Hold::new(1.0);
        h.enabled = false;
        h.start_hold();
        assert!(!h.active);
        assert!(!h.just_began);
    }

    #[test]
    fn tick_advances_hold_time() {
        let mut h = Hold::new(2.0);
        h.start_hold();
        h.tick(1.0);
        assert!((h.hold_time - 1.0).abs() < 1e-5);
    }

    #[test]
    fn tick_caps_hold_time_at_max() {
        let mut h = Hold::new(1.0);
        h.start_hold();
        h.tick(5.0);
        assert!((h.hold_time - 1.0).abs() < 1e-5);
    }

    #[test]
    fn tick_no_advance_when_not_holding() {
        let mut h = Hold::new(1.0);
        h.tick(0.5);
        assert_eq!(h.hold_time, 0.0);
    }

    #[test]
    fn tick_clears_just_began() {
        let mut h = Hold::new(1.0);
        h.start_hold();
        h.tick(0.016);
        assert!(!h.just_began);
    }

    #[test]
    fn release_returns_power_fraction() {
        let mut h = Hold::new(2.0);
        h.start_hold();
        h.tick(1.0); // half charged
        let p = h.release();
        assert!((p - 0.5).abs() < 1e-5);
    }

    #[test]
    fn release_returns_one_at_full_charge() {
        let mut h = Hold::new(1.0);
        h.start_hold();
        h.tick(1.0);
        let p = h.release();
        assert!((p - 1.0).abs() < 1e-5);
    }

    #[test]
    fn release_returns_zero_when_not_holding() {
        let mut h = Hold::new(1.0);
        let p = h.release();
        assert_eq!(p, 0.0);
    }

    #[test]
    fn release_returns_zero_when_disabled() {
        let mut h = Hold::new(1.0);
        h.start_hold(); // bypass disable guard
        h.enabled = false;
        let p = h.release();
        assert_eq!(p, 0.0);
    }

    #[test]
    fn release_fires_just_released() {
        let mut h = Hold::new(1.0);
        h.start_hold();
        h.tick(0.5);
        h.release();
        assert!(h.just_released);
    }

    #[test]
    fn release_resets_hold_time_and_active() {
        let mut h = Hold::new(1.0);
        h.start_hold();
        h.tick(0.8);
        h.release();
        assert!(!h.active);
        assert_eq!(h.hold_time, 0.0);
    }

    #[test]
    fn tick_clears_just_released() {
        let mut h = Hold::new(1.0);
        h.start_hold();
        h.release();
        h.tick(0.016);
        assert!(!h.just_released);
    }

    #[test]
    fn power_fraction_zero_at_start() {
        let mut h = Hold::new(1.0);
        h.start_hold();
        assert_eq!(h.power_fraction(), 0.0);
    }

    #[test]
    fn power_fraction_scales_with_time() {
        let mut h = Hold::new(4.0);
        h.start_hold();
        h.tick(1.0); // 0.25
        assert!((h.power_fraction() - 0.25).abs() < 1e-5);
        h.tick(1.0); // 0.5
        assert!((h.power_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn is_fully_charged_true_at_max() {
        let mut h = Hold::new(1.0);
        h.start_hold();
        h.tick(1.5);
        assert!(h.is_fully_charged());
    }

    #[test]
    fn is_fully_charged_false_when_not_holding() {
        let h = Hold::new(1.0);
        assert!(!h.is_fully_charged());
    }

    #[test]
    fn max_hold_time_clamped_to_min() {
        let h = Hold::new(0.0);
        assert!(h.max_hold_time >= 0.001);
    }

    #[test]
    fn can_restart_after_release() {
        let mut h = Hold::new(1.0);
        h.start_hold();
        h.tick(1.0);
        h.release();
        h.tick(0.016);
        h.start_hold();
        assert!(h.active);
        assert!(h.just_began);
    }
}
