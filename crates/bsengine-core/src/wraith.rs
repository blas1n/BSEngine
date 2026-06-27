use bevy_ecs::prelude::Component;

/// Spectral form that makes an entity partially non-corporeal for a fixed
/// duration. While in wraith form, incoming damage is reduced by
/// `damage_reduction`. The form begins via `enter_wraith()` and ends either
/// naturally when `wraith_timer` reaches zero or immediately via
/// `exit_wraith()`.
///
/// `enter_wraith()` sets `in_wraith_form = true`, resets `wraith_timer` to
/// `wraith_duration`, and fires `just_entered`. No-op when already in wraith
/// form or disabled.
///
/// `exit_wraith()` immediately ends wraith form, zeros the timer, and fires
/// `just_exited`. No-op when not in wraith form.
///
/// `tick(dt)` clears one-frame flags at start; when in wraith form counts
/// down `wraith_timer`; fires `just_exited` and clears `in_wraith_form` on
/// natural expiry. No-op when disabled.
///
/// `is_wraith()` returns `in_wraith_form && enabled`.
///
/// `effective_incoming(base)` returns
/// `base * (1.0 - damage_reduction)` when `is_wraith()`, floored at 0.0;
/// returns `base` otherwise.
///
/// Distinct from `Ghost` (permanent untargetable invisibility),
/// `Invincible` (full immunity, no reduction), `Phase` (pass-through movement
/// with no damage modifier), and `Spectral` (aesthetic/VFX component with no
/// gameplay stats): Wraith is a **timed partial-immunity form** — it reduces
/// but does not negate damage, and ends naturally or on demand.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wraith {
    /// Remaining seconds of wraith form. 0.0 when inactive.
    pub wraith_timer: f32,
    /// Duration of each wraith form activation. Clamped >= 0.1.
    pub wraith_duration: f32,
    /// Fraction of incoming damage blocked while in wraith form.
    /// Clamped [0.0, 1.0].
    pub damage_reduction: f32,
    pub in_wraith_form: bool,
    pub just_entered: bool,
    pub just_exited: bool,
    pub enabled: bool,
}

impl Wraith {
    pub fn new(wraith_duration: f32, damage_reduction: f32) -> Self {
        Self {
            wraith_timer: 0.0,
            wraith_duration: wraith_duration.max(0.1),
            damage_reduction: damage_reduction.clamp(0.0, 1.0),
            in_wraith_form: false,
            just_entered: false,
            just_exited: false,
            enabled: true,
        }
    }

    /// Begin wraith form for `wraith_duration` seconds. Fires `just_entered`.
    /// No-op when already in wraith form or disabled.
    pub fn enter_wraith(&mut self) {
        if !self.enabled || self.in_wraith_form {
            return;
        }
        self.in_wraith_form = true;
        self.wraith_timer = self.wraith_duration;
        self.just_entered = true;
    }

    /// End wraith form immediately. Zeros the timer and fires `just_exited`.
    /// No-op when not in wraith form.
    pub fn exit_wraith(&mut self) {
        if !self.in_wraith_form {
            return;
        }
        self.in_wraith_form = false;
        self.wraith_timer = 0.0;
        self.just_exited = true;
    }

    /// Advance the wraith timer. Clears one-frame flags first; counts down
    /// `wraith_timer`; fires `just_exited` and clears `in_wraith_form` on
    /// natural expiry. No-op when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_entered = false;
        self.just_exited = false;

        if !self.enabled {
            return;
        }

        if self.in_wraith_form {
            self.wraith_timer = (self.wraith_timer - dt).max(0.0);
            if self.wraith_timer == 0.0 {
                self.in_wraith_form = false;
                self.just_exited = true;
            }
        }
    }

    /// `true` when in wraith form and the component is enabled.
    pub fn is_wraith(&self) -> bool {
        self.in_wraith_form && self.enabled
    }

    /// Incoming damage reduced by wraith form. Returns
    /// `base * (1.0 - damage_reduction)` when `is_wraith()`, floored at 0.0;
    /// returns `base` otherwise.
    pub fn effective_incoming(&self, base: f32) -> f32 {
        if self.is_wraith() {
            (base * (1.0 - self.damage_reduction)).max(0.0)
        } else {
            base
        }
    }
}

impl Default for Wraith {
    fn default() -> Self {
        Self::new(3.0, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_inactive() {
        let w = Wraith::new(3.0, 0.5);
        assert!(!w.in_wraith_form);
        assert!(!w.is_wraith());
        assert_eq!(w.wraith_timer, 0.0);
    }

    #[test]
    fn enter_wraith_sets_form() {
        let mut w = Wraith::new(3.0, 0.5);
        w.enter_wraith();
        assert!(w.in_wraith_form);
        assert!(w.is_wraith());
    }

    #[test]
    fn enter_wraith_sets_timer_to_duration() {
        let mut w = Wraith::new(3.0, 0.5);
        w.enter_wraith();
        assert!((w.wraith_timer - 3.0).abs() < 1e-5);
    }

    #[test]
    fn enter_wraith_fires_just_entered() {
        let mut w = Wraith::new(3.0, 0.5);
        w.enter_wraith();
        assert!(w.just_entered);
    }

    #[test]
    fn enter_wraith_no_op_when_already_in_form() {
        let mut w = Wraith::new(3.0, 0.5);
        w.enter_wraith();
        w.tick(1.0); // advance timer, clear flags
        w.enter_wraith(); // should not reset timer
        assert!((w.wraith_timer - 2.0).abs() < 1e-3);
        assert!(!w.just_entered);
    }

    #[test]
    fn enter_wraith_no_op_when_disabled() {
        let mut w = Wraith::new(3.0, 0.5);
        w.enabled = false;
        w.enter_wraith();
        assert!(!w.in_wraith_form);
    }

    #[test]
    fn exit_wraith_clears_form() {
        let mut w = Wraith::new(3.0, 0.5);
        w.enter_wraith();
        w.exit_wraith();
        assert!(!w.in_wraith_form);
        assert!(!w.is_wraith());
    }

    #[test]
    fn exit_wraith_zeros_timer() {
        let mut w = Wraith::new(3.0, 0.5);
        w.enter_wraith();
        w.exit_wraith();
        assert_eq!(w.wraith_timer, 0.0);
    }

    #[test]
    fn exit_wraith_fires_just_exited() {
        let mut w = Wraith::new(3.0, 0.5);
        w.enter_wraith();
        w.exit_wraith();
        assert!(w.just_exited);
    }

    #[test]
    fn exit_wraith_no_op_when_not_in_form() {
        let mut w = Wraith::new(3.0, 0.5);
        w.exit_wraith(); // should not panic
        assert!(!w.just_exited);
    }

    #[test]
    fn tick_counts_down_timer() {
        let mut w = Wraith::new(3.0, 0.5);
        w.enter_wraith();
        w.tick(1.0);
        assert!((w.wraith_timer - 2.0).abs() < 1e-5);
        assert!(w.in_wraith_form);
    }

    #[test]
    fn tick_expires_form_at_zero() {
        let mut w = Wraith::new(2.0, 0.5);
        w.enter_wraith();
        w.tick(2.0);
        assert!(!w.in_wraith_form);
        assert!(!w.is_wraith());
    }

    #[test]
    fn tick_fires_just_exited_on_expiry() {
        let mut w = Wraith::new(1.0, 0.5);
        w.enter_wraith();
        w.tick(1.0);
        assert!(w.just_exited);
    }

    #[test]
    fn tick_no_just_exited_while_still_active() {
        let mut w = Wraith::new(3.0, 0.5);
        w.enter_wraith();
        w.tick(1.0);
        assert!(!w.just_exited);
    }

    #[test]
    fn tick_clears_just_entered_next_frame() {
        let mut w = Wraith::new(3.0, 0.5);
        w.enter_wraith();
        assert!(w.just_entered);
        w.tick(0.016);
        assert!(!w.just_entered);
    }

    #[test]
    fn tick_clears_just_exited_next_frame() {
        let mut w = Wraith::new(1.0, 0.5);
        w.enter_wraith();
        w.tick(1.0); // just_exited = true
        w.tick(0.016); // cleared
        assert!(!w.just_exited);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = Wraith::new(2.0, 0.5);
        w.enter_wraith();
        w.enabled = false;
        w.tick(2.0);
        // timer not counted down because disabled
        assert!((w.wraith_timer - 2.0).abs() < 1e-5);
    }

    #[test]
    fn tick_no_advance_when_not_in_form() {
        let mut w = Wraith::new(3.0, 0.5);
        w.tick(1.0); // not in form, timer stays 0
        assert_eq!(w.wraith_timer, 0.0);
    }

    #[test]
    fn is_wraith_false_when_disabled() {
        let mut w = Wraith::new(3.0, 0.5);
        w.in_wraith_form = true;
        w.enabled = false;
        assert!(!w.is_wraith());
    }

    #[test]
    fn effective_incoming_reduced_while_wraith() {
        let mut w = Wraith::new(3.0, 0.5);
        w.enter_wraith();
        // 100 * (1 - 0.5) = 50
        assert!((w.effective_incoming(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_incoming_base_when_not_in_form() {
        let w = Wraith::new(3.0, 0.5);
        assert!((w.effective_incoming(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_incoming_base_when_disabled() {
        let mut w = Wraith::new(3.0, 0.5);
        w.enter_wraith();
        w.enabled = false;
        assert!((w.effective_incoming(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_incoming_floored_at_zero() {
        let mut w = Wraith::new(3.0, 1.0); // 100% reduction
        w.enter_wraith();
        assert_eq!(w.effective_incoming(100.0), 0.0);
    }

    #[test]
    fn effective_incoming_partial_reduction() {
        let mut w = Wraith::new(3.0, 0.3);
        w.enter_wraith();
        // 100 * (1 - 0.3) = 70
        assert!((w.effective_incoming(100.0) - 70.0).abs() < 1e-3);
    }

    #[test]
    fn wraith_duration_clamped_to_min() {
        let w = Wraith::new(0.0, 0.5);
        assert!((w.wraith_duration - 0.1).abs() < 1e-5);
    }

    #[test]
    fn damage_reduction_clamped_to_one() {
        let w = Wraith::new(3.0, 2.0);
        assert!((w.damage_reduction - 1.0).abs() < 1e-5);
    }

    #[test]
    fn damage_reduction_clamped_to_zero() {
        let w = Wraith::new(3.0, -0.5);
        assert_eq!(w.damage_reduction, 0.0);
    }

    #[test]
    fn re_enter_after_expiry_fires_just_entered_again() {
        let mut w = Wraith::new(1.0, 0.5);
        w.enter_wraith();
        w.tick(1.0); // expired
        w.tick(0.016); // clear flags
        w.enter_wraith(); // re-enter
        assert!(w.just_entered);
        assert!(w.is_wraith());
    }

    #[test]
    fn re_enter_after_manual_exit() {
        let mut w = Wraith::new(3.0, 0.5);
        w.enter_wraith();
        w.exit_wraith();
        w.tick(0.016); // clear flags
        w.enter_wraith();
        assert!(w.in_wraith_form);
        assert!((w.wraith_timer - 3.0).abs() < 1e-5);
    }

    #[test]
    fn manual_exit_fires_just_exited_independently_of_tick() {
        let mut w = Wraith::new(3.0, 0.5);
        w.enter_wraith();
        w.tick(0.016); // clear just_entered
        w.exit_wraith(); // fires just_exited outside tick
        assert!(w.just_exited);
        assert!(!w.in_wraith_form);
    }
}
