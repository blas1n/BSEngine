use bevy_ecs::prelude::Component;

/// Mild disorientation CC: reduces movement speed and aim accuracy without
/// fully stopping the entity (unlike `Stun`).
///
/// The movement system multiplies speed by `(1.0 - slow_fraction)` while dazed.
/// Aimed attacks should add random angular noise up to `aim_deviation_rad`.
/// Common after heavy hits in melee-heavy games (e.g. "stunned but can still move").
///
/// `apply(duration)` uses high-watermark; `tick(dt)` counts down and sets
/// `just_undazed` when the effect ends.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Daze {
    pub duration: f32,
    pub timer: f32,
    /// Movement speed reduction fraction [0.0, 1.0]. 0.3 = 30% slower.
    pub slow_fraction: f32,
    /// Maximum random aim deviation in radians while dazed.
    pub aim_deviation_rad: f32,
    pub just_dazed: bool,
    pub just_undazed: bool,
    pub enabled: bool,
}

impl Daze {
    pub fn new(slow_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            slow_fraction: slow_fraction.clamp(0.0, 1.0),
            aim_deviation_rad: 0.0,
            just_dazed: false,
            just_undazed: false,
            enabled: true,
        }
    }

    pub fn with_aim_deviation(mut self, radians: f32) -> Self {
        self.aim_deviation_rad = radians.max(0.0);
        self
    }

    /// Apply or extend a daze of `duration` seconds. High-watermark: only
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
                self.just_dazed = true;
            }
        }
    }

    /// Remove the effect immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_undazed = true;
        }
    }

    /// Advance the timer; sets `just_undazed` when the effect expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_dazed = false;
        self.just_undazed = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_undazed = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective speed multiplier while dazed. Returns 1.0 when not active.
    pub fn speed_multiplier(&self) -> f32 {
        if self.is_active() {
            1.0 - self.slow_fraction
        } else {
            1.0
        }
    }

    /// Fraction of the daze duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Daze {
    fn default() -> Self {
        Self::new(0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_daze() {
        let mut d = Daze::new(0.3);
        d.apply(2.0);
        assert!(d.is_active());
        assert!(d.just_dazed);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut d = Daze::new(0.3);
        d.apply(2.0);
        d.tick(0.016);
        d.apply(5.0);
        assert!((d.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut d = Daze::new(0.3);
        d.apply(5.0);
        d.apply(2.0);
        assert!((d.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_daze() {
        let mut d = Daze::new(0.3);
        d.apply(1.0);
        d.tick(1.1);
        assert!(!d.is_active());
        assert!(d.just_undazed);
    }

    #[test]
    fn clear_ends_daze_early() {
        let mut d = Daze::new(0.3);
        d.apply(5.0);
        d.clear();
        assert!(!d.is_active());
        assert!(d.just_undazed);
    }

    #[test]
    fn speed_multiplier_while_dazed() {
        let mut d = Daze::new(0.4);
        d.apply(3.0);
        let m = d.speed_multiplier();
        assert!((m - 0.6).abs() < 1e-5);
    }

    #[test]
    fn speed_multiplier_when_inactive() {
        let d = Daze::new(0.4);
        assert!((d.speed_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut d = Daze::new(0.3);
        d.apply(2.0);
        d.tick(1.0);
        let frac = d.remaining_fraction();
        assert!((frac - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut d = Daze::new(0.3);
        d.enabled = false;
        d.apply(5.0);
        assert!(!d.is_active());
    }
}
