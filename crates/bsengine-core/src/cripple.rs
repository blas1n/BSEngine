use bevy_ecs::prelude::Component;

/// Severe leg-injury CC that dramatically cuts movement speed and optionally
/// prevents jumping.
///
/// `effective_move_speed(base)` returns `base * speed_fraction` while active.
/// When `prevents_jump` is true, jump systems check `is_active()` and skip
/// activation. `apply(duration)` uses high-watermark.
///
/// Distinct from `Hobble` (moderate speed penalty, blocks dash/sprint but not
/// jump; debuff flavour): Cripple is a harder CC representing a severe or
/// debilitating injury — the entity can barely move and cannot leave the
/// ground.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Cripple {
    pub duration: f32,
    pub timer: f32,
    /// Fraction [0.0, 1.0] of base move speed available while crippled.
    pub speed_fraction: f32,
    /// If true, jump systems skip activation while crippled.
    pub prevents_jump: bool,
    pub just_crippled: bool,
    pub just_recovered: bool,
    pub enabled: bool,
}

impl Cripple {
    pub fn new(speed_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            speed_fraction: speed_fraction.clamp(0.0, 1.0),
            prevents_jump: true,
            just_crippled: false,
            just_recovered: false,
            enabled: true,
        }
    }

    pub fn with_prevent_jump(mut self, prevents: bool) -> Self {
        self.prevents_jump = prevents;
        self
    }

    /// Apply or extend the cripple for `duration` seconds. High-watermark:
    /// only replaces the current timer if the new duration is longer.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_crippled = true;
            }
        }
    }

    /// Remove the cripple immediately (e.g. from a heal or antidote).
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_recovered = true;
        }
    }

    /// Advance the timer; sets `just_recovered` when the debuff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_crippled = false;
        self.just_recovered = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_recovered = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective move speed after applying the cripple.
    /// Returns `base * speed_fraction` while active, `base` otherwise.
    pub fn effective_move_speed(&self, base: f32) -> f32 {
        if self.is_active() {
            base * self.speed_fraction
        } else {
            base
        }
    }

    /// Fraction of the cripple duration remaining [1.0 = just applied, 0.0 = recovered].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Cripple {
    fn default() -> Self {
        Self::new(0.2).with_prevent_jump(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_cripple() {
        let mut c = Cripple::new(0.2);
        c.apply(3.0);
        assert!(c.is_active());
        assert!(c.just_crippled);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut c = Cripple::new(0.2);
        c.apply(2.0);
        c.tick(0.016);
        c.apply(5.0);
        assert!((c.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut c = Cripple::new(0.2);
        c.apply(5.0);
        c.apply(2.0);
        assert!((c.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_cripple() {
        let mut c = Cripple::new(0.2);
        c.apply(1.0);
        c.tick(1.1);
        assert!(!c.is_active());
        assert!(c.just_recovered);
    }

    #[test]
    fn clear_ends_early() {
        let mut c = Cripple::new(0.2);
        c.apply(5.0);
        c.clear();
        assert!(!c.is_active());
        assert!(c.just_recovered);
    }

    #[test]
    fn effective_move_speed_while_active() {
        let mut c = Cripple::new(0.2);
        c.apply(3.0);
        let speed = c.effective_move_speed(10.0);
        assert!((speed - 2.0).abs() < 1e-4); // 10 * 0.2
    }

    #[test]
    fn effective_move_speed_when_inactive() {
        let c = Cripple::new(0.2);
        assert!((c.effective_move_speed(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn prevents_jump_default_true() {
        let c = Cripple::new(0.2);
        assert!(c.prevents_jump);
    }

    #[test]
    fn prevents_jump_can_be_disabled() {
        let c = Cripple::new(0.2).with_prevent_jump(false);
        assert!(!c.prevents_jump);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut c = Cripple::new(0.2);
        c.apply(2.0);
        c.tick(1.0);
        assert!((c.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut c = Cripple::new(0.2);
        c.enabled = false;
        c.apply(5.0);
        assert!(!c.is_active());
    }

    #[test]
    fn tick_clears_just_crippled() {
        let mut c = Cripple::new(0.2);
        c.apply(3.0);
        c.tick(0.016);
        assert!(!c.just_crippled);
    }
}
