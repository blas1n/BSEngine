use bevy_ecs::prelude::Component;

/// Concealment-by-obscurity debuff that reduces enemies' effective detection
/// range against this entity.
///
/// While hazed, any enemy AI or detection system should multiply its normal
/// detection range by `effective_detection_range(enemy_range)` when checking
/// whether this entity is visible. A value of 0.0 makes the entity undetectable;
/// 1.0 leaves detection range unchanged.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_cleared` when the haze dissipates.
///
/// Distinct from `Blind` (the entity itself cannot see far), `Stealth` (full
/// invisibility toggle), and `Shroud` (blocks a specific ability category):
/// Haze is passive environmental concealment — the entity remains partially
/// visible but enemies must be much closer to detect it.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Haze {
    pub duration: f32,
    pub timer: f32,
    /// Multiplier applied to enemy detection range. Clamped to [0.0, 1.0].
    /// e.g. 0.4 = enemies can only detect this entity at 40% of their normal range.
    pub detection_range_fraction: f32,
    pub just_hazed: bool,
    pub just_cleared: bool,
    pub enabled: bool,
}

impl Haze {
    pub fn new(detection_range_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            detection_range_fraction: detection_range_fraction.clamp(0.0, 1.0),
            just_hazed: false,
            just_cleared: false,
            enabled: true,
        }
    }

    /// Apply or extend the haze for `duration` seconds. High-watermark: only
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
                self.just_hazed = true;
            }
        }
    }

    /// Remove the haze immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_cleared = true;
        }
    }

    /// Advance the timer; sets `just_cleared` when the haze dissipates.
    pub fn tick(&mut self, dt: f32) {
        self.just_hazed = false;
        self.just_cleared = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_cleared = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Enemy's effective detection range against this entity.
    /// Returns `enemy_range * detection_range_fraction` while active,
    /// `enemy_range` otherwise.
    pub fn effective_detection_range(&self, enemy_range: f32) -> f32 {
        if self.is_active() {
            enemy_range * self.detection_range_fraction
        } else {
            enemy_range
        }
    }

    /// Fraction of the haze duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Haze {
    fn default() -> Self {
        Self::new(0.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_haze() {
        let mut h = Haze::new(0.4);
        h.apply(3.0);
        assert!(h.is_active());
        assert!(h.just_hazed);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut h = Haze::new(0.4);
        h.apply(2.0);
        h.tick(0.016);
        h.apply(5.0);
        assert!((h.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut h = Haze::new(0.4);
        h.apply(5.0);
        h.apply(2.0);
        assert!((h.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_haze() {
        let mut h = Haze::new(0.4);
        h.apply(1.0);
        h.tick(1.1);
        assert!(!h.is_active());
        assert!(h.just_cleared);
    }

    #[test]
    fn clear_ends_early() {
        let mut h = Haze::new(0.4);
        h.apply(5.0);
        h.clear();
        assert!(!h.is_active());
        assert!(h.just_cleared);
    }

    #[test]
    fn effective_detection_range_while_active() {
        let mut h = Haze::new(0.4);
        h.apply(3.0);
        assert!((h.effective_detection_range(100.0) - 40.0).abs() < 1e-3);
    }

    #[test]
    fn effective_detection_range_when_inactive() {
        let h = Haze::new(0.4);
        assert!((h.effective_detection_range(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut h = Haze::new(0.4);
        h.apply(2.0);
        h.tick(1.0);
        assert!((h.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut h = Haze::new(0.4);
        h.enabled = false;
        h.apply(5.0);
        assert!(!h.is_active());
    }

    #[test]
    fn tick_clears_just_hazed() {
        let mut h = Haze::new(0.4);
        h.apply(3.0);
        h.tick(0.016);
        assert!(!h.just_hazed);
    }

    #[test]
    fn detection_range_fraction_clamped() {
        let h = Haze::new(1.5);
        assert!((h.detection_range_fraction - 1.0).abs() < 1e-5);
        let h2 = Haze::new(-0.2);
        assert!((h2.detection_range_fraction - 0.0).abs() < 1e-5);
    }
}
