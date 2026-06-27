use bevy_ecs::prelude::Component;

/// Single-shot countdown detonator: arm once, triggers once when the timer reaches zero.
///
/// Call `arm()` to start the countdown. Each frame `tick(dt)` advances the timer
/// and returns `true` exactly once — on the frame the fuse fires. After triggering,
/// the fuse is automatically disarmed and `just_triggered` is set for one frame.
///
/// Typical uses: explosive charges, delayed ability activations, breakable traps,
/// countdown sequences. Distinct from `Cooldown` (repeatable rate limit) and
/// `Timer` (general reusable timer): `Fuse` arms, counts down, and fires once.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Fuse {
    /// Duration of the countdown in seconds.
    pub duration: f32,
    /// Remaining time; `> 0.0` while armed.
    pub timer: f32,
    /// True on the single frame when the fuse detonates.
    pub just_triggered: bool,
    pub enabled: bool,
}

impl Fuse {
    pub fn new(duration: f32) -> Self {
        Self {
            duration: duration.max(0.0),
            timer: 0.0,
            just_triggered: false,
            enabled: true,
        }
    }

    /// Arm the fuse and start the countdown. No-op if already armed.
    pub fn arm(&mut self) {
        if !self.enabled || self.is_armed() {
            return;
        }
        self.timer = self.duration;
        self.just_triggered = false;
    }

    /// Arm the fuse with a custom `duration`, overriding the stored one.
    pub fn arm_with(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }
        self.duration = duration.max(0.0);
        self.timer = self.duration;
        self.just_triggered = false;
    }

    /// Disarm the fuse without triggering it.
    pub fn disarm(&mut self) {
        self.timer = 0.0;
        self.just_triggered = false;
    }

    /// Advance the countdown. Returns `true` exactly once, on the frame the fuse fires.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.just_triggered = false;

        if !self.enabled || !self.is_armed() {
            return false;
        }

        self.timer -= dt;
        if self.timer <= 0.0 {
            self.timer = 0.0;
            self.just_triggered = true;
            return true;
        }

        false
    }

    pub fn is_armed(&self) -> bool {
        self.timer > 0.0
    }

    /// Fraction of the countdown remaining [1.0 = just armed, 0.0 = triggered].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Fuse {
    fn default() -> Self {
        Self::new(3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arm_starts_countdown() {
        let mut f = Fuse::new(5.0);
        f.arm();
        assert!(f.is_armed());
        assert!((f.timer - 5.0).abs() < 1e-5);
    }

    #[test]
    fn arm_no_op_when_already_armed() {
        let mut f = Fuse::new(5.0);
        f.arm();
        f.tick(1.0);
        let before = f.timer;
        f.arm(); // should not reset
        assert!((f.timer - before).abs() < 1e-5);
    }

    #[test]
    fn arm_with_custom_duration() {
        let mut f = Fuse::new(5.0);
        f.arm_with(10.0);
        assert!((f.timer - 10.0).abs() < 1e-5);
    }

    #[test]
    fn disarm_stops_countdown() {
        let mut f = Fuse::new(5.0);
        f.arm();
        f.disarm();
        assert!(!f.is_armed());
    }

    #[test]
    fn tick_returns_false_while_counting() {
        let mut f = Fuse::new(5.0);
        f.arm();
        assert!(!f.tick(1.0));
        assert!(!f.just_triggered);
    }

    #[test]
    fn tick_returns_true_on_expiry() {
        let mut f = Fuse::new(1.0);
        f.arm();
        let result = f.tick(1.1);
        assert!(result);
        assert!(f.just_triggered);
        assert!(!f.is_armed());
    }

    #[test]
    fn tick_returns_false_after_trigger() {
        let mut f = Fuse::new(1.0);
        f.arm();
        f.tick(2.0); // trigger
        let result = f.tick(0.016); // next frame
        assert!(!result);
        assert!(!f.just_triggered);
    }

    #[test]
    fn tick_no_trigger_when_unarmed() {
        let mut f = Fuse::new(1.0);
        assert!(!f.tick(5.0));
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut f = Fuse::new(2.0);
        f.arm();
        f.tick(1.0);
        assert!((f.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_arm_no_op() {
        let mut f = Fuse::new(5.0);
        f.enabled = false;
        f.arm();
        assert!(!f.is_armed());
    }

    #[test]
    fn disabled_tick_no_trigger() {
        let mut f = Fuse::new(1.0);
        f.arm();
        f.enabled = false;
        assert!(!f.tick(2.0));
    }
}
