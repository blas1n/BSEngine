use bevy_ecs::prelude::Component;

/// Instant-death countdown: when doomed, the entity dies when `countdown`
/// reaches 0 regardless of remaining HP. The death system should call
/// `tick(dt)` every frame and trigger death on `just_expired`.
///
/// `doom(duration)` starts or extends the timer using a high-watermark: only
/// the longer of the current remaining time and the new duration takes effect.
/// Sets `just_doomed` on the first application (inactive → active transition).
/// `cleanse()` removes the doom early. `tick(dt)` counts down and sets
/// `just_expired` when the timer reaches 0.
///
/// `doom(duration)` is a no-op when disabled or `duration ≤ 0`.
///
/// Distinct from `Curse` (stat penalties), `Wither` (HP drain over time), and
/// `Poison` (periodic damage): Doom is an **instant-death countdown** — HP
/// does not matter once the timer expires.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Doom {
    pub active: bool,
    /// Remaining seconds until death. Counts down to 0.
    pub countdown: f32,
    /// Original duration of the current doom application. Used for fraction.
    pub max_countdown: f32,
    pub just_doomed: bool,
    pub just_expired: bool,
    pub enabled: bool,
}

impl Doom {
    pub fn new() -> Self {
        Self {
            active: false,
            countdown: 0.0,
            max_countdown: 0.0,
            just_doomed: false,
            just_expired: false,
            enabled: true,
        }
    }

    /// Apply (or extend) doom for `duration` seconds. High-watermark: only
    /// replaces the remaining countdown when `duration > countdown`. Sets
    /// `just_doomed` on the inactive → active transition. No-op when disabled
    /// or `duration ≤ 0`.
    pub fn doom(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        let was_doomed = self.is_doomed();
        if duration > self.countdown {
            self.countdown = duration;
            self.max_countdown = duration;
        }
        if !was_doomed {
            self.active = true;
            self.just_doomed = true;
        }
    }

    /// Remove doom early. No-op when not doomed.
    pub fn cleanse(&mut self) {
        if !self.is_doomed() {
            return;
        }
        self.active = false;
        self.countdown = 0.0;
        self.max_countdown = 0.0;
    }

    /// Advance the doom countdown by `dt` seconds. Sets `just_expired` when
    /// the countdown reaches 0. Clears one-frame flags at the start of each
    /// tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_doomed = false;
        self.just_expired = false;

        if self.active && self.countdown > 0.0 {
            self.countdown -= dt;
            if self.countdown <= 0.0 {
                self.countdown = 0.0;
                self.active = false;
                self.just_expired = true;
            }
        }
    }

    /// `true` while the countdown is actively running.
    pub fn is_doomed(&self) -> bool {
        self.active && self.countdown > 0.0
    }

    /// Fraction of doom time remaining [1.0 = just applied, 0.0 = expired].
    /// Returns 0.0 when not doomed.
    pub fn time_fraction(&self) -> f32 {
        if self.max_countdown <= 0.0 || !self.is_doomed() {
            return 0.0;
        }
        (self.countdown / self.max_countdown).clamp(0.0, 1.0)
    }
}

impl Default for Doom {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_not_doomed() {
        let d = Doom::new();
        assert!(!d.is_doomed());
        assert!(!d.just_doomed);
        assert!(!d.just_expired);
    }

    #[test]
    fn doom_activates() {
        let mut d = Doom::new();
        d.doom(5.0);
        assert!(d.is_doomed());
        assert!(d.just_doomed);
        assert!((d.countdown - 5.0).abs() < 1e-5);
    }

    #[test]
    fn doom_no_op_when_disabled() {
        let mut d = Doom::new();
        d.enabled = false;
        d.doom(5.0);
        assert!(!d.is_doomed());
        assert!(!d.just_doomed);
    }

    #[test]
    fn doom_no_op_when_duration_zero_or_negative() {
        let mut d = Doom::new();
        d.doom(0.0);
        assert!(!d.is_doomed());
        d.doom(-1.0);
        assert!(!d.is_doomed());
    }

    #[test]
    fn doom_high_watermark_extends_on_longer() {
        let mut d = Doom::new();
        d.doom(3.0);
        d.tick(1.0); // countdown = 2.0
        d.doom(5.0); // 5.0 > 2.0 → replaces
        assert!((d.countdown - 5.0).abs() < 1e-3);
    }

    #[test]
    fn doom_high_watermark_no_shrink() {
        let mut d = Doom::new();
        d.doom(5.0);
        d.doom(2.0); // shorter → ignored
        assert!((d.countdown - 5.0).abs() < 1e-5);
    }

    #[test]
    fn doom_no_just_doomed_on_extend() {
        let mut d = Doom::new();
        d.doom(3.0);
        d.tick(0.016);
        d.doom(8.0); // extends, not first doom
        assert!(!d.just_doomed);
    }

    #[test]
    fn cleanse_removes_doom() {
        let mut d = Doom::new();
        d.doom(5.0);
        d.cleanse();
        assert!(!d.is_doomed());
        assert_eq!(d.countdown, 0.0);
    }

    #[test]
    fn cleanse_no_op_when_not_doomed() {
        let mut d = Doom::new();
        d.cleanse(); // no panic
        assert!(!d.is_doomed());
    }

    #[test]
    fn tick_counts_down() {
        let mut d = Doom::new();
        d.doom(5.0);
        d.tick(2.0);
        assert!((d.countdown - 3.0).abs() < 1e-3);
        assert!(d.is_doomed());
    }

    #[test]
    fn tick_expires_and_fires_just_expired() {
        let mut d = Doom::new();
        d.doom(2.0);
        d.tick(2.5);
        assert!(!d.is_doomed());
        assert!(d.just_expired);
        assert_eq!(d.countdown, 0.0);
    }

    #[test]
    fn tick_clears_just_doomed() {
        let mut d = Doom::new();
        d.doom(5.0);
        d.tick(0.016);
        assert!(!d.just_doomed);
    }

    #[test]
    fn tick_clears_just_expired() {
        let mut d = Doom::new();
        d.doom(1.0);
        d.tick(2.0); // expires
        d.tick(0.016);
        assert!(!d.just_expired);
    }

    #[test]
    fn tick_no_op_when_not_doomed() {
        let mut d = Doom::new();
        d.tick(5.0);
        assert!(!d.is_doomed());
        assert_eq!(d.countdown, 0.0);
    }

    #[test]
    fn time_fraction_at_full() {
        let mut d = Doom::new();
        d.doom(4.0);
        assert!((d.time_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn time_fraction_at_half() {
        let mut d = Doom::new();
        d.doom(4.0);
        d.tick(2.0);
        assert!((d.time_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn time_fraction_zero_when_not_doomed() {
        let d = Doom::new();
        assert_eq!(d.time_fraction(), 0.0);
    }

    #[test]
    fn can_redoom_after_cleanse() {
        let mut d = Doom::new();
        d.doom(3.0);
        d.cleanse();
        d.doom(5.0);
        assert!(d.is_doomed());
        assert!(d.just_doomed);
    }

    #[test]
    fn can_redoom_after_expiry() {
        let mut d = Doom::new();
        d.doom(1.0);
        d.tick(2.0); // expires
        d.tick(0.016); // clears flags
        d.doom(3.0);
        assert!(d.is_doomed());
        assert!(d.just_doomed);
    }
}
