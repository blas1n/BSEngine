use bevy_ecs::prelude::Component;

/// Unanimous-approval gate. Tracks affirmative votes toward a `required`
/// threshold. When enough votes accumulate, the motion passes. Votes can be
/// rescinded, revoking passage. Models group consensus, cooperative triggers,
/// ritual completion, or any multi-party agreement mechanic.
///
/// `vote()` increments `count`. Fires `just_passed` when `count` first
/// reaches `required`. No-op when disabled or already passed.
///
/// `revoke()` decrements `count`. Fires `just_revoked` when count drops
/// below `required` (passage reversed). No-op when disabled or count is
/// already 0.
///
/// `tick(_dt)` clears `just_passed` and `just_revoked` only.
///
/// `is_passed()` returns `count >= required && enabled`.
///
/// `vote_fraction()` returns `(count / required).clamp(0, 1)`.
///
/// `effective_power(base)` returns `base` when `is_passed()`; `0.0`
/// otherwise.
///
/// Default: `new(3)`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yea {
    pub count: u32,
    pub required: u32,
    pub just_passed: bool,
    pub just_revoked: bool,
    pub enabled: bool,
}

impl Yea {
    pub fn new(required: u32) -> Self {
        Self {
            count: 0,
            required: required.max(1),
            just_passed: false,
            just_revoked: false,
            enabled: true,
        }
    }

    /// Add one vote. Fires `just_passed` when reaching `required`. No-op
    /// when disabled or already passed.
    pub fn vote(&mut self) {
        if !self.enabled || self.count >= self.required {
            return;
        }
        self.count += 1;
        if self.count >= self.required {
            self.just_passed = true;
        }
    }

    /// Remove one vote. Fires `just_revoked` when dropping below `required`
    /// (reversing passage). No-op when disabled or count is already 0.
    pub fn revoke(&mut self) {
        if !self.enabled || self.count == 0 {
            return;
        }
        let was_passed = self.count >= self.required;
        self.count -= 1;
        if was_passed && self.count < self.required {
            self.just_revoked = true;
        }
    }

    /// Advance one frame: clear `just_passed` and `just_revoked` only.
    pub fn tick(&mut self, _dt: f32) {
        self.just_passed = false;
        self.just_revoked = false;
    }

    /// `true` when vote count meets or exceeds `required` and component is enabled.
    pub fn is_passed(&self) -> bool {
        self.count >= self.required && self.enabled
    }

    /// Fraction of required votes cast [0.0, 1.0].
    pub fn vote_fraction(&self) -> f32 {
        (self.count as f32 / self.required as f32).clamp(0.0, 1.0)
    }

    /// Returns `base` when `is_passed()`; `0.0` otherwise.
    pub fn effective_power(&self, base: f32) -> f32 {
        if self.is_passed() {
            base
        } else {
            0.0
        }
    }
}

impl Default for Yea {
    fn default() -> Self {
        Self::new(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yea {
        Yea::new(3)
    }

    // --- construction ---

    #[test]
    fn new_starts_at_zero() {
        let y = y();
        assert_eq!(y.count, 0);
        assert_eq!(y.required, 3);
        assert!(!y.is_passed());
    }

    #[test]
    fn new_clamps_required_to_one() {
        let y = Yea::new(0);
        assert_eq!(y.required, 1);
    }

    #[test]
    fn default_required_is_three() {
        assert_eq!(Yea::default().required, 3);
    }

    // --- vote ---

    #[test]
    fn vote_increments_count() {
        let mut y = y();
        y.vote();
        assert_eq!(y.count, 1);
    }

    #[test]
    fn vote_fires_just_passed_at_required() {
        let mut y = y();
        y.vote();
        y.vote();
        y.vote();
        assert!(y.just_passed);
        assert!(y.is_passed());
    }

    #[test]
    fn vote_no_op_when_already_passed() {
        let mut y = y();
        y.vote();
        y.vote();
        y.vote();
        y.tick(0.016);
        y.vote();
        assert_eq!(y.count, 3); // capped
        assert!(!y.just_passed);
    }

    #[test]
    fn vote_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.vote();
        assert_eq!(y.count, 0);
    }

    // --- revoke ---

    #[test]
    fn revoke_decrements_count() {
        let mut y = y();
        y.vote();
        y.vote();
        y.revoke();
        assert_eq!(y.count, 1);
    }

    #[test]
    fn revoke_fires_just_revoked_when_reversing_passage() {
        let mut y = y();
        y.vote();
        y.vote();
        y.vote(); // passed
        y.tick(0.016);
        y.revoke(); // drops below required
        assert!(y.just_revoked);
        assert!(!y.is_passed());
    }

    #[test]
    fn revoke_does_not_fire_just_revoked_below_threshold() {
        let mut y = y();
        y.vote();
        y.vote();
        y.revoke(); // count drops from 2 to 1 — never passed
        assert!(!y.just_revoked);
    }

    #[test]
    fn revoke_no_op_when_count_zero() {
        let mut y = y();
        y.revoke();
        assert_eq!(y.count, 0);
        assert!(!y.just_revoked);
    }

    #[test]
    fn revoke_no_op_when_disabled() {
        let mut y = y();
        y.vote();
        y.enabled = false;
        y.revoke();
        assert_eq!(y.count, 1);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_passed() {
        let mut y = y();
        y.vote();
        y.vote();
        y.vote();
        y.tick(0.016);
        assert!(!y.just_passed);
    }

    #[test]
    fn tick_clears_just_revoked() {
        let mut y = y();
        y.vote();
        y.vote();
        y.vote();
        y.revoke();
        y.tick(0.016);
        assert!(!y.just_revoked);
    }

    #[test]
    fn tick_does_not_change_count() {
        let mut y = y();
        y.vote();
        y.vote();
        y.tick(1000.0);
        assert_eq!(y.count, 2);
    }

    // --- is_passed ---

    #[test]
    fn is_passed_false_below_required() {
        let mut y = y();
        y.vote();
        y.vote();
        assert!(!y.is_passed());
    }

    #[test]
    fn is_passed_true_at_required() {
        let mut y = y();
        y.vote();
        y.vote();
        y.vote();
        assert!(y.is_passed());
    }

    #[test]
    fn is_passed_false_when_disabled() {
        let mut y = y();
        y.vote();
        y.vote();
        y.vote();
        y.enabled = false;
        assert!(!y.is_passed());
    }

    // --- vote_fraction ---

    #[test]
    fn vote_fraction_zero_at_start() {
        assert_eq!(y().vote_fraction(), 0.0);
    }

    #[test]
    fn vote_fraction_two_thirds_at_two_of_three() {
        let mut y = y();
        y.vote();
        y.vote();
        assert!((y.vote_fraction() - 2.0 / 3.0).abs() < 1e-4);
    }

    #[test]
    fn vote_fraction_one_when_passed() {
        let mut y = y();
        y.vote();
        y.vote();
        y.vote();
        assert!((y.vote_fraction() - 1.0).abs() < 1e-5);
    }

    // --- effective_power ---

    #[test]
    fn effective_power_zero_before_passed() {
        let mut y = y();
        y.vote();
        assert_eq!(y.effective_power(100.0), 0.0);
    }

    #[test]
    fn effective_power_full_when_passed() {
        let mut y = y();
        y.vote();
        y.vote();
        y.vote();
        assert!((y.effective_power(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_power_zero_when_disabled() {
        let mut y = y();
        y.vote();
        y.vote();
        y.vote();
        y.enabled = false;
        assert_eq!(y.effective_power(100.0), 0.0);
    }
}
