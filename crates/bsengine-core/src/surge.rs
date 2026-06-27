use bevy_ecs::prelude::Component;

/// Temporary power spike that amplifies all outgoing damage and healing by
/// `multiplier`.
///
/// Surge represents a brief window of heightened combat effectiveness — a
/// berserker frenzy, an overclocked weapon system, or a divine empowerment.
/// Unlike `Buff` (which manages multiple keyed entries), `Surge` is a single
/// multiplier that the damage/healing pipeline checks before applying numbers.
///
/// `apply(duration)` uses high-watermark: a shorter re-application is ignored.
/// `tick(dt)` counts down and sets `just_expired` when the surge ends.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Surge {
    pub duration: f32,
    pub timer: f32,
    /// Output multiplier applied while surging (e.g. 1.5 = +50% damage/healing).
    pub multiplier: f32,
    pub just_surged: bool,
    pub just_expired: bool,
    pub enabled: bool,
}

impl Surge {
    pub fn new(multiplier: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            multiplier: multiplier.max(0.0),
            just_surged: false,
            just_expired: false,
            enabled: true,
        }
    }

    /// Apply or extend a surge of `duration` seconds. High-watermark: only
    /// replaces the current timer if the new duration is longer.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_surged = true;
            }
        }
    }

    /// Cancel the surge immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_expired = true;
        }
    }

    /// Advance the timer; sets `just_expired` when the surge ends.
    pub fn tick(&mut self, dt: f32) {
        self.just_surged = false;
        self.just_expired = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_expired = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective output multiplier: `multiplier` while surging, `1.0` otherwise.
    pub fn damage_multiplier(&self) -> f32 {
        if self.is_active() {
            self.multiplier
        } else {
            1.0
        }
    }

    /// Fraction of the surge duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Surge {
    fn default() -> Self {
        Self::new(1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_surge() {
        let mut s = Surge::new(1.5);
        s.apply(3.0);
        assert!(s.is_active());
        assert!(s.just_surged);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut s = Surge::new(1.5);
        s.apply(2.0);
        s.tick(0.016);
        s.apply(5.0);
        assert!((s.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut s = Surge::new(1.5);
        s.apply(5.0);
        s.apply(2.0);
        assert!((s.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_surge() {
        let mut s = Surge::new(1.5);
        s.apply(1.0);
        s.tick(1.1);
        assert!(!s.is_active());
        assert!(s.just_expired);
    }

    #[test]
    fn clear_cancels_surge() {
        let mut s = Surge::new(1.5);
        s.apply(5.0);
        s.clear();
        assert!(!s.is_active());
        assert!(s.just_expired);
    }

    #[test]
    fn damage_multiplier_while_active() {
        let mut s = Surge::new(2.0);
        s.apply(3.0);
        assert!((s.damage_multiplier() - 2.0).abs() < 1e-5);
    }

    #[test]
    fn damage_multiplier_when_inactive() {
        let s = Surge::new(2.0);
        assert!((s.damage_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut s = Surge::new(1.5);
        s.apply(2.0);
        s.tick(1.0);
        let frac = s.remaining_fraction();
        assert!((frac - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut s = Surge::new(1.5);
        s.enabled = false;
        s.apply(5.0);
        assert!(!s.is_active());
    }

    #[test]
    fn tick_clears_just_surged() {
        let mut s = Surge::new(1.5);
        s.apply(3.0);
        s.tick(0.016);
        assert!(!s.just_surged);
    }
}
