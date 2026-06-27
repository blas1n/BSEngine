use bevy_ecs::prelude::Component;

/// Sustained active melee defense: while fending, the entity keeps attackers
/// at bay with weapon pressure, reducing the rate of successful incoming melee
/// hits by `fend_efficiency`.
///
/// `fend(duration)` starts or extends the stance with a high-watermark timer:
/// only replaces the current timer when `duration > timer`. Fires `just_began`
/// on the inactive → active transition. No-op when disabled or `duration ≤ 0`.
///
/// `lower()` ends the stance early and fires `just_ended`. `tick(dt)` counts
/// down and fires `just_ended` on natural expiry. One-frame flags are cleared
/// at the start of each `tick` call.
///
/// `hit_chance(base)` returns `(base * (1 - fend_efficiency)).max(0.0)` while
/// fending and enabled; returns `base` otherwise. Combat systems call this to
/// determine whether a melee swing connects.
///
/// Distinct from `Parry` (momentary perfect block that stuns the attacker),
/// `Deflect` (redirects projectiles), and `Guard` (reduces raw damage from a
/// chosen direction): Fend is a **sustained pressure defense** — it does not
/// block or redirect individual hits, but suppresses how often they land during
/// the fend window.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Fend {
    pub active: bool,
    pub timer: f32,
    /// Fraction of melee hit rate suppressed while fending. Clamped [0.0, 1.0].
    pub fend_efficiency: f32,
    pub just_began: bool,
    pub just_ended: bool,
    pub enabled: bool,
}

impl Fend {
    pub fn new(fend_efficiency: f32) -> Self {
        Self {
            active: false,
            timer: 0.0,
            fend_efficiency: fend_efficiency.clamp(0.0, 1.0),
            just_began: false,
            just_ended: false,
            enabled: true,
        }
    }

    /// Begin or extend the fend stance for `duration` seconds.
    /// High-watermark: only replaces the current timer when
    /// `duration > timer`. Fires `just_began` on the inactive → active
    /// transition. No-op when disabled or `duration ≤ 0`.
    pub fn fend(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_active = self.active;
            self.timer = duration;
            self.active = true;
            if !was_active {
                self.just_began = true;
            }
        }
    }

    /// Drop the fend stance early. Fires `just_ended`. No-op when not fending.
    pub fn lower(&mut self) {
        if !self.active {
            return;
        }
        self.timer = 0.0;
        self.active = false;
        self.just_ended = true;
    }

    /// Advance the fend timer. Fires `just_ended` when the stance expires.
    /// Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_began = false;
        self.just_ended = false;

        if self.active && self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.active = false;
                self.just_ended = true;
            }
        }
    }

    /// `true` while actively fending and enabled.
    pub fn is_fending(&self) -> bool {
        self.active && self.enabled
    }

    /// Effective incoming melee hit chance after fend suppression.
    /// Returns `(base * (1 - fend_efficiency)).max(0.0)` while fending and
    /// enabled; returns `base` otherwise.
    pub fn hit_chance(&self, base: f32) -> f32 {
        if self.is_fending() {
            (base * (1.0 - self.fend_efficiency)).max(0.0)
        } else {
            base
        }
    }
}

impl Default for Fend {
    fn default() -> Self {
        Self::new(0.6)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_not_fending() {
        let f = Fend::new(0.6);
        assert!(!f.is_fending());
        assert_eq!(f.timer, 0.0);
    }

    #[test]
    fn fend_activates() {
        let mut f = Fend::new(0.6);
        f.fend(3.0);
        assert!(f.is_fending());
        assert!(f.just_began);
        assert!((f.timer - 3.0).abs() < 1e-5);
    }

    #[test]
    fn fend_extends_on_longer_duration() {
        let mut f = Fend::new(0.6);
        f.fend(2.0);
        f.tick(0.5);
        f.fend(5.0);
        assert!((f.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn fend_no_extend_on_shorter_duration() {
        let mut f = Fend::new(0.6);
        f.fend(5.0);
        f.fend(2.0);
        assert!((f.timer - 5.0).abs() < 1e-5);
    }

    #[test]
    fn just_began_not_set_on_extend() {
        let mut f = Fend::new(0.6);
        f.fend(2.0);
        f.tick(0.016);
        f.fend(5.0);
        assert!(!f.just_began);
    }

    #[test]
    fn fend_no_op_when_disabled() {
        let mut f = Fend::new(0.6);
        f.enabled = false;
        f.fend(3.0);
        assert!(!f.active);
    }

    #[test]
    fn fend_no_op_when_duration_zero() {
        let mut f = Fend::new(0.6);
        f.fend(0.0);
        assert!(!f.active);
    }

    #[test]
    fn fend_no_op_when_duration_negative() {
        let mut f = Fend::new(0.6);
        f.fend(-1.0);
        assert!(!f.active);
    }

    #[test]
    fn lower_ends_fend() {
        let mut f = Fend::new(0.6);
        f.fend(3.0);
        f.lower();
        assert!(!f.is_fending());
        assert!(f.just_ended);
        assert_eq!(f.timer, 0.0);
    }

    #[test]
    fn lower_no_op_when_not_fending() {
        let mut f = Fend::new(0.6);
        f.lower();
        assert!(!f.just_ended);
    }

    #[test]
    fn tick_counts_down() {
        let mut f = Fend::new(0.6);
        f.fend(5.0);
        f.tick(2.0);
        assert!((f.timer - 3.0).abs() < 1e-4);
        assert!(f.is_fending());
    }

    #[test]
    fn tick_expires_fend() {
        let mut f = Fend::new(0.6);
        f.fend(2.0);
        f.tick(2.5);
        assert!(!f.is_fending());
        assert!(f.just_ended);
        assert_eq!(f.timer, 0.0);
    }

    #[test]
    fn tick_clears_just_began() {
        let mut f = Fend::new(0.6);
        f.fend(3.0);
        f.tick(0.016);
        assert!(!f.just_began);
    }

    #[test]
    fn tick_clears_just_ended() {
        let mut f = Fend::new(0.6);
        f.fend(1.0);
        f.tick(2.0); // expires
        f.tick(0.016);
        assert!(!f.just_ended);
    }

    #[test]
    fn is_fending_false_when_disabled() {
        let mut f = Fend::new(0.6);
        f.fend(3.0);
        f.enabled = false;
        assert!(!f.is_fending());
    }

    #[test]
    fn hit_chance_reduced_while_fending() {
        let mut f = Fend::new(0.5);
        f.fend(3.0);
        // 100 * (1 - 0.5) = 50
        assert!((f.hit_chance(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn hit_chance_floored_at_zero() {
        let mut f = Fend::new(1.0);
        f.fend(3.0);
        assert!((f.hit_chance(100.0)).abs() < 1e-5);
    }

    #[test]
    fn hit_chance_base_when_not_fending() {
        let f = Fend::new(0.6);
        assert!((f.hit_chance(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn hit_chance_base_when_disabled() {
        let mut f = Fend::new(0.6);
        f.fend(3.0);
        f.enabled = false;
        assert!((f.hit_chance(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn fend_efficiency_clamped_at_one() {
        let f = Fend::new(2.0);
        assert!((f.fend_efficiency - 1.0).abs() < 1e-5);
    }

    #[test]
    fn fend_efficiency_clamped_at_zero() {
        let f = Fend::new(-0.5);
        assert_eq!(f.fend_efficiency, 0.0);
    }

    #[test]
    fn can_re_fend_after_lower() {
        let mut f = Fend::new(0.6);
        f.fend(2.0);
        f.lower();
        f.tick(0.016);
        f.fend(3.0);
        assert!(f.is_fending());
        assert!(f.just_began);
    }

    #[test]
    fn can_re_fend_after_expiry() {
        let mut f = Fend::new(0.6);
        f.fend(1.0);
        f.tick(2.0); // expires
        f.tick(0.016); // clear flags
        f.fend(3.0);
        assert!(f.is_fending());
        assert!(f.just_began);
    }
}
