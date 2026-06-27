use bevy_ecs::prelude::Component;

/// Zoom / aim-down-sights state: binary precision mode that increases accuracy
/// and effective range at the cost of reduced movement speed.
///
/// `scope_in()` activates the mode and fires `just_scoped` on the inactive →
/// active transition. `scope_out()` deactivates it and fires `just_unscoped`.
/// `tick()` clears one-frame flags. Both are no-ops when already in the
/// requested state. `scope_in()` is additionally a no-op when disabled.
///
/// While `is_scoped()`:
/// - `effective_range(base)` returns `base * (1 + range_bonus)`
/// - `effective_move_speed(base)` returns `base * (1 - move_speed_penalty)`,
///   floored at 0.0
///
/// Distinct from `Aim` (continuous targeting direction), `Rifle` (the ranged
/// weapon component), and `Focus` (concentration/spell-channelling mechanic):
/// Scope is the **zoom/ADS binary state** — on/off precision mode that trades
/// mobility for accuracy and range.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Scope {
    pub active: bool,
    /// Added accuracy fraction while scoped. Clamped ≥ 0.0.
    pub accuracy_bonus: f32,
    /// Range multiplier added while scoped. Clamped ≥ 0.0.
    pub range_bonus: f32,
    /// Fraction of movement speed lost while scoped. Clamped [0.0, 1.0].
    pub move_speed_penalty: f32,
    pub just_scoped: bool,
    pub just_unscoped: bool,
    pub enabled: bool,
}

impl Scope {
    pub fn new(accuracy_bonus: f32, range_bonus: f32, move_speed_penalty: f32) -> Self {
        Self {
            active: false,
            accuracy_bonus: accuracy_bonus.max(0.0),
            range_bonus: range_bonus.max(0.0),
            move_speed_penalty: move_speed_penalty.clamp(0.0, 1.0),
            just_scoped: false,
            just_unscoped: false,
            enabled: true,
        }
    }

    /// Enter zoom / ADS mode. Fires `just_scoped` on the inactive → active
    /// transition. No-op when already scoped or disabled.
    pub fn scope_in(&mut self) {
        if !self.enabled || self.active {
            return;
        }
        self.active = true;
        self.just_scoped = true;
    }

    /// Leave zoom / ADS mode. Fires `just_unscoped`. No-op when not scoped.
    pub fn scope_out(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        self.just_unscoped = true;
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_scoped = false;
        self.just_unscoped = false;
    }

    /// `true` while zoomed in and enabled.
    pub fn is_scoped(&self) -> bool {
        self.active && self.enabled
    }

    /// Effective range while scoped: `base * (1 + range_bonus)`.
    /// Returns `base` when not scoped or disabled.
    pub fn effective_range(&self, base: f32) -> f32 {
        if self.is_scoped() {
            base * (1.0 + self.range_bonus)
        } else {
            base
        }
    }

    /// Effective movement speed while scoped:
    /// `base * (1 - move_speed_penalty)`, floored at 0.0.
    /// Returns `base` when not scoped or disabled.
    pub fn effective_move_speed(&self, base: f32) -> f32 {
        if self.is_scoped() {
            (base * (1.0 - self.move_speed_penalty)).max(0.0)
        } else {
            base
        }
    }
}

impl Default for Scope {
    fn default() -> Self {
        Self::new(0.3, 0.5, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_not_scoped() {
        let s = Scope::new(0.3, 0.5, 0.5);
        assert!(!s.is_scoped());
        assert!(!s.just_scoped);
    }

    #[test]
    fn scope_in_activates() {
        let mut s = Scope::new(0.3, 0.5, 0.5);
        s.scope_in();
        assert!(s.is_scoped());
        assert!(s.just_scoped);
    }

    #[test]
    fn scope_in_no_op_when_already_scoped() {
        let mut s = Scope::new(0.3, 0.5, 0.5);
        s.scope_in();
        s.tick();
        s.scope_in();
        assert!(!s.just_scoped);
    }

    #[test]
    fn scope_in_no_op_when_disabled() {
        let mut s = Scope::new(0.3, 0.5, 0.5);
        s.enabled = false;
        s.scope_in();
        assert!(!s.active);
    }

    #[test]
    fn scope_out_deactivates() {
        let mut s = Scope::new(0.3, 0.5, 0.5);
        s.scope_in();
        s.scope_out();
        assert!(!s.is_scoped());
        assert!(s.just_unscoped);
    }

    #[test]
    fn scope_out_no_op_when_not_scoped() {
        let mut s = Scope::new(0.3, 0.5, 0.5);
        s.scope_out();
        assert!(!s.just_unscoped);
    }

    #[test]
    fn scope_out_works_when_disabled() {
        let mut s = Scope::new(0.3, 0.5, 0.5);
        s.scope_in();
        s.enabled = false;
        s.scope_out(); // active=true even though is_scoped()=false; should still clear
        assert!(!s.active);
        assert!(s.just_unscoped);
    }

    #[test]
    fn tick_clears_just_scoped() {
        let mut s = Scope::new(0.3, 0.5, 0.5);
        s.scope_in();
        s.tick();
        assert!(!s.just_scoped);
    }

    #[test]
    fn tick_clears_just_unscoped() {
        let mut s = Scope::new(0.3, 0.5, 0.5);
        s.scope_in();
        s.scope_out();
        s.tick();
        assert!(!s.just_unscoped);
    }

    #[test]
    fn is_scoped_false_when_disabled() {
        let mut s = Scope::new(0.3, 0.5, 0.5);
        s.scope_in();
        s.enabled = false;
        assert!(!s.is_scoped());
    }

    #[test]
    fn effective_range_scales_when_scoped() {
        let mut s = Scope::new(0.3, 0.5, 0.5);
        s.scope_in();
        // 100 * (1 + 0.5) = 150
        assert!((s.effective_range(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_range_base_when_not_scoped() {
        let s = Scope::new(0.3, 0.5, 0.5);
        assert!((s.effective_range(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_move_speed_reduced_when_scoped() {
        let mut s = Scope::new(0.3, 0.5, 0.5);
        s.scope_in();
        // 100 * (1 - 0.5) = 50
        assert!((s.effective_move_speed(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_move_speed_floored_at_zero() {
        let mut s = Scope::new(0.3, 0.5, 1.0); // full penalty
        s.scope_in();
        assert!((s.effective_move_speed(100.0)).abs() < 1e-5);
    }

    #[test]
    fn effective_move_speed_base_when_not_scoped() {
        let s = Scope::new(0.3, 0.5, 0.5);
        assert!((s.effective_move_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn accuracy_bonus_clamped_non_negative() {
        let s = Scope::new(-1.0, 0.5, 0.5);
        assert_eq!(s.accuracy_bonus, 0.0);
    }

    #[test]
    fn range_bonus_clamped_non_negative() {
        let s = Scope::new(0.3, -0.5, 0.5);
        assert_eq!(s.range_bonus, 0.0);
    }

    #[test]
    fn move_speed_penalty_clamped() {
        let s = Scope::new(0.3, 0.5, 2.0);
        assert!((s.move_speed_penalty - 1.0).abs() < 1e-5);
    }

    #[test]
    fn can_re_scope_after_scope_out() {
        let mut s = Scope::new(0.3, 0.5, 0.5);
        s.scope_in();
        s.scope_out();
        s.tick();
        s.scope_in();
        assert!(s.is_scoped());
        assert!(s.just_scoped);
    }

    #[test]
    fn effective_range_base_when_disabled() {
        let mut s = Scope::new(0.3, 0.5, 0.5);
        s.scope_in();
        s.enabled = false;
        assert!((s.effective_range(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_move_speed_base_when_disabled() {
        let mut s = Scope::new(0.3, 0.5, 0.5);
        s.scope_in();
        s.enabled = false;
        assert!((s.effective_move_speed(100.0) - 100.0).abs() < 1e-5);
    }
}
