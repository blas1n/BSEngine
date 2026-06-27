use bevy_ecs::prelude::Component;

/// Caster-side ability to hold a target motionless at range. The entity that
/// carries `Transfix` is the one doing the locking — this is the **caster's**
/// perspective, not the victim's. While `is_transfixing()`, the caster
/// maintains eye contact (or equivalent) that keeps the target locked;
/// breaking range or running the timer to zero ends the hold.
///
/// `fix(duration)` starts or extends the transfix window (high-watermark:
/// only replaces the timer when `duration > timer`). Fires `just_locked` on
/// the inactive → active transition. No-op when disabled or `duration ≤ 0`.
///
/// `break_lock()` ends the transfix early. Fires `just_broken`. No-op when
/// not transfixing.
///
/// `tick(dt)` clears one-frame flags at the start, then counts down the
/// timer. Fires `just_broken` when the timer expires naturally.
///
/// `is_transfixing()` returns `active && enabled`.
///
/// `in_range(dist)` returns `dist ≤ lock_range && enabled` — helper for
/// systems to determine if the target is still close enough to maintain the
/// lock.
///
/// Distinct from `Entangle` (wraps the *victim* in physical restraints),
/// `Root` (pins the *victim* in place via ground effects), `Snare` (slows or
/// traps the *victim*), and `Stun` (stuns the *victim*): Transfix is a
/// **caster-owned ranged immobilisation ability** — the entity carrying this
/// component is the one projecting the lock, not the one being locked.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Transfix {
    pub active: bool,
    pub timer: f32,
    /// Maximum distance to sustain the lock. Clamped > 0.
    pub lock_range: f32,
    pub just_locked: bool,
    pub just_broken: bool,
    pub enabled: bool,
}

impl Transfix {
    pub fn new(lock_range: f32) -> Self {
        Self {
            active: false,
            timer: 0.0,
            lock_range: lock_range.max(0.001),
            just_locked: false,
            just_broken: false,
            enabled: true,
        }
    }

    /// Start or extend the transfix (high-watermark: only replaces the timer
    /// when `duration > timer`). Fires `just_locked` on the inactive → active
    /// transition. No-op when disabled or `duration ≤ 0`.
    pub fn fix(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_inactive = !self.is_transfixing();
            self.timer = duration;
            self.active = true;
            if was_inactive {
                self.just_locked = true;
            }
        }
    }

    /// End the transfix early. Fires `just_broken`. No-op when not
    /// transfixing.
    pub fn break_lock(&mut self) {
        if !self.is_transfixing() {
            return;
        }
        self.active = false;
        self.timer = 0.0;
        self.just_broken = true;
    }

    /// Advance the transfix timer. Clears one-frame flags at start. Fires
    /// `just_broken` when the timer expires naturally.
    pub fn tick(&mut self, dt: f32) {
        self.just_locked = false;
        self.just_broken = false;

        if self.active {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.active = false;
                self.just_broken = true;
            }
        }
    }

    /// `true` when the entity is actively transfixing and the component is
    /// enabled.
    pub fn is_transfixing(&self) -> bool {
        self.active && self.enabled
    }

    /// `true` when `dist ≤ lock_range` and the component is enabled.
    /// Systems call this each frame to determine whether the target is still
    /// close enough to sustain the lock.
    pub fn in_range(&self, dist: f32) -> bool {
        self.enabled && dist <= self.lock_range
    }

    /// Remaining timer fraction relative to a known original duration.
    /// Returns `(timer / original_duration).clamp(0, 1)`; 0.0 when inactive
    /// or `original_duration ≤ 0`.
    pub fn remaining_fraction(&self, original_duration: f32) -> f32 {
        if !self.active || original_duration <= 0.0 {
            return 0.0;
        }
        (self.timer / original_duration).clamp(0.0, 1.0)
    }
}

impl Default for Transfix {
    fn default() -> Self {
        Self::new(8.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_inactive() {
        let t = Transfix::new(8.0);
        assert!(!t.active);
        assert!(!t.is_transfixing());
    }

    #[test]
    fn fix_starts_transfix() {
        let mut t = Transfix::new(8.0);
        t.fix(3.0);
        assert!(t.active);
        assert!(t.just_locked);
        assert!(t.is_transfixing());
    }

    #[test]
    fn fix_extends_on_longer_duration() {
        let mut t = Transfix::new(8.0);
        t.fix(2.0);
        t.tick(0.016);
        t.fix(10.0);
        assert!((t.timer - 10.0).abs() < 1e-4);
    }

    #[test]
    fn fix_no_extend_on_shorter_duration() {
        let mut t = Transfix::new(8.0);
        t.fix(10.0);
        t.fix(3.0);
        assert!((t.timer - 10.0).abs() < 1e-4);
    }

    #[test]
    fn just_locked_not_set_on_extend() {
        let mut t = Transfix::new(8.0);
        t.fix(3.0);
        t.tick(0.016);
        t.fix(10.0);
        assert!(!t.just_locked);
    }

    #[test]
    fn fix_no_op_when_disabled() {
        let mut t = Transfix::new(8.0);
        t.enabled = false;
        t.fix(5.0);
        assert!(!t.active);
    }

    #[test]
    fn fix_no_op_at_zero_duration() {
        let mut t = Transfix::new(8.0);
        t.fix(0.0);
        assert!(!t.active);
    }

    #[test]
    fn break_lock_ends_transfix() {
        let mut t = Transfix::new(8.0);
        t.fix(5.0);
        t.break_lock();
        assert!(!t.active);
        assert!(t.just_broken);
        assert!(!t.is_transfixing());
    }

    #[test]
    fn break_lock_no_op_when_inactive() {
        let mut t = Transfix::new(8.0);
        t.break_lock();
        assert!(!t.just_broken);
    }

    #[test]
    fn break_lock_no_op_when_disabled() {
        let mut t = Transfix::new(8.0);
        t.fix(5.0);
        t.tick(0.016);
        t.enabled = false;
        t.break_lock();
        assert!(!t.just_broken);
    }

    #[test]
    fn tick_expires_naturally() {
        let mut t = Transfix::new(8.0);
        t.fix(1.0);
        t.tick(0.016);
        t.tick(2.0);
        assert!(!t.active);
        assert!(t.just_broken);
    }

    #[test]
    fn tick_clears_just_locked() {
        let mut t = Transfix::new(8.0);
        t.fix(5.0);
        t.tick(0.016);
        assert!(!t.just_locked);
    }

    #[test]
    fn tick_clears_just_broken() {
        let mut t = Transfix::new(8.0);
        t.fix(0.5);
        t.tick(0.016);
        t.tick(1.0); // expires
        t.tick(0.016);
        assert!(!t.just_broken);
    }

    #[test]
    fn is_transfixing_false_when_disabled() {
        let mut t = Transfix::new(8.0);
        t.fix(5.0);
        t.enabled = false;
        assert!(!t.is_transfixing());
    }

    #[test]
    fn in_range_within_lock_range() {
        let t = Transfix::new(8.0);
        assert!(t.in_range(5.0));
    }

    #[test]
    fn in_range_at_exact_lock_range() {
        let t = Transfix::new(8.0);
        assert!(t.in_range(8.0));
    }

    #[test]
    fn in_range_false_beyond_lock_range() {
        let t = Transfix::new(8.0);
        assert!(!t.in_range(9.0));
    }

    #[test]
    fn in_range_false_when_disabled() {
        let mut t = Transfix::new(8.0);
        t.enabled = false;
        assert!(!t.in_range(3.0));
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut t = Transfix::new(8.0);
        t.fix(4.0);
        t.tick(2.0);
        assert!((t.remaining_fraction(4.0) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_zero_when_inactive() {
        let t = Transfix::new(8.0);
        assert_eq!(t.remaining_fraction(5.0), 0.0);
    }

    #[test]
    fn remaining_fraction_zero_at_zero_original() {
        let mut t = Transfix::new(8.0);
        t.fix(5.0);
        assert_eq!(t.remaining_fraction(0.0), 0.0);
    }

    #[test]
    fn lock_range_clamped_above_zero() {
        let t = Transfix::new(0.0);
        assert!(t.lock_range > 0.0);
    }

    #[test]
    fn re_locks_after_natural_expiry() {
        let mut t = Transfix::new(8.0);
        t.fix(0.5);
        t.tick(0.016);
        t.tick(1.0);
        t.tick(0.016);
        t.fix(3.0);
        assert!(t.just_locked);
    }

    #[test]
    fn re_locks_after_break_lock() {
        let mut t = Transfix::new(8.0);
        t.fix(5.0);
        t.break_lock();
        t.tick(0.016);
        t.fix(3.0);
        assert!(t.just_locked);
    }
}
