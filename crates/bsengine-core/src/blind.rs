use bevy_ecs::prelude::Component;

/// CC status effect that reduces an entity's effective vision and aim accuracy.
///
/// While blinded, the entity's vision range is capped at `range_limit` (world
/// units) and aimed attacks should apply a random angular deviation up to
/// `aim_deviation_rad` radians. A value of 0.0 for `range_limit` means the
/// entity is completely blind (no vision at all).
///
/// `apply(duration)` extends if the new duration is longer (high-watermark).
/// `tick(dt)` counts down and clears the effect, setting `just_unblinded` for
/// animation/sound hooks.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Blind {
    pub duration: f32,
    pub timer: f32,
    /// Maximum vision range while blinded. 0.0 = total blindness.
    pub range_limit: f32,
    /// Maximum random aim deviation in radians while blinded.
    pub aim_deviation_rad: f32,
    pub just_blinded: bool,
    pub just_unblinded: bool,
    pub enabled: bool,
}

impl Blind {
    /// Create a default blind that halves vision range and adds 30° max deviation.
    pub fn new() -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            range_limit: 5.0,
            aim_deviation_rad: std::f32::consts::FRAC_PI_6, // 30 degrees
            just_blinded: false,
            just_unblinded: false,
            enabled: true,
        }
    }

    pub fn with_range_limit(mut self, limit: f32) -> Self {
        self.range_limit = limit.max(0.0);
        self
    }

    pub fn with_aim_deviation(mut self, radians: f32) -> Self {
        self.aim_deviation_rad = radians.max(0.0);
        self
    }

    /// Apply or extend a blind of `duration` seconds. High-watermark: only
    /// replaces the current duration if the new one is longer.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_blinded = true;
            }
        }
    }

    /// Remove the effect immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_unblinded = true;
        }
    }

    /// Advance the timer; sets `just_unblinded` when the effect expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_blinded = false;
        self.just_unblinded = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_unblinded = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Fraction of the blind duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Blind {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_blind() {
        let mut b = Blind::new();
        b.apply(3.0);
        assert!(b.is_active());
        assert!(b.just_blinded);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut b = Blind::new();
        b.apply(2.0);
        b.tick(0.016); // clear just_blinded
        b.apply(5.0);
        assert!((b.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut b = Blind::new();
        b.apply(5.0);
        b.apply(2.0);
        assert!((b.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_blind() {
        let mut b = Blind::new();
        b.apply(1.0);
        b.tick(1.1);
        assert!(!b.is_active());
        assert!(b.just_unblinded);
    }

    #[test]
    fn clear_ends_blind_early() {
        let mut b = Blind::new();
        b.apply(5.0);
        b.clear();
        assert!(!b.is_active());
        assert!(b.just_unblinded);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut b = Blind::new();
        b.apply(2.0);
        b.tick(1.0);
        let frac = b.remaining_fraction();
        assert!((frac - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut b = Blind::new();
        b.enabled = false;
        b.apply(5.0);
        assert!(!b.is_active());
    }

    #[test]
    fn tick_clears_just_blinded() {
        let mut b = Blind::new();
        b.apply(3.0);
        b.tick(0.016);
        assert!(!b.just_blinded);
    }
}
