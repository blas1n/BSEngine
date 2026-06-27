use bevy_ecs::prelude::Component;

/// Involuntary-shaking debuff that reduces aim precision and disrupts actions.
///
/// While active, VFX and animation systems should apply a shake of `intensity`
/// magnitude. Ability and attack systems can use `aim_deviation_rad()` to add
/// a random angular offset to aimed projectiles.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_stopped` when the debuff expires.
///
/// More persistent than `Flinch` (single-frame reaction), more disruptive than
/// `Daze` (aim penalty only): Tremble combines a visual shake with a meaningful
/// aim-accuracy reduction that lasts for a duration.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Tremble {
    pub duration: f32,
    pub timer: f32,
    /// Shaking severity [0.0, 1.0]. Drives both VFX amplitude and aim deviation.
    pub intensity: f32,
    pub just_trembling: bool,
    pub just_stopped: bool,
    pub enabled: bool,
}

impl Tremble {
    pub fn new(intensity: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            intensity: intensity.clamp(0.0, 1.0),
            just_trembling: false,
            just_stopped: false,
            enabled: true,
        }
    }

    /// Apply or extend the tremble for `duration` seconds. High-watermark:
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
                self.just_trembling = true;
            }
        }
    }

    /// End the tremble immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_stopped = true;
        }
    }

    /// Advance the timer; sets `just_stopped` when the debuff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_trembling = false;
        self.just_stopped = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_stopped = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Maximum random aim deviation in radians driven by `intensity`.
    /// Scales linearly: `intensity * π/4` (up to 45° at full intensity).
    /// Returns `0.0` when inactive.
    pub fn aim_deviation_rad(&self) -> f32 {
        if self.is_active() {
            self.intensity * std::f32::consts::FRAC_PI_4
        } else {
            0.0
        }
    }

    /// Fraction of the tremble duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Tremble {
    fn default() -> Self {
        Self::new(0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_tremble() {
        let mut t = Tremble::new(0.5);
        t.apply(2.0);
        assert!(t.is_active());
        assert!(t.just_trembling);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut t = Tremble::new(0.5);
        t.apply(2.0);
        t.tick(0.016);
        t.apply(5.0);
        assert!((t.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut t = Tremble::new(0.5);
        t.apply(5.0);
        t.apply(2.0);
        assert!((t.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_tremble() {
        let mut t = Tremble::new(0.5);
        t.apply(1.0);
        t.tick(1.1);
        assert!(!t.is_active());
        assert!(t.just_stopped);
    }

    #[test]
    fn clear_ends_early() {
        let mut t = Tremble::new(0.5);
        t.apply(5.0);
        t.clear();
        assert!(!t.is_active());
        assert!(t.just_stopped);
    }

    #[test]
    fn aim_deviation_while_active() {
        let mut t = Tremble::new(1.0);
        t.apply(3.0);
        let expected = std::f32::consts::FRAC_PI_4;
        assert!((t.aim_deviation_rad() - expected).abs() < 1e-5);
    }

    #[test]
    fn aim_deviation_when_inactive() {
        let t = Tremble::new(1.0);
        assert!((t.aim_deviation_rad() - 0.0).abs() < 1e-5);
    }

    #[test]
    fn aim_deviation_scales_with_intensity() {
        let mut t = Tremble::new(0.5);
        t.apply(3.0);
        let expected = 0.5 * std::f32::consts::FRAC_PI_4;
        assert!((t.aim_deviation_rad() - expected).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut t = Tremble::new(0.5);
        t.apply(2.0);
        t.tick(1.0);
        assert!((t.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut t = Tremble::new(0.5);
        t.enabled = false;
        t.apply(5.0);
        assert!(!t.is_active());
    }

    #[test]
    fn tick_clears_just_trembling() {
        let mut t = Tremble::new(0.5);
        t.apply(3.0);
        t.tick(0.016);
        assert!(!t.just_trembling);
    }
}
