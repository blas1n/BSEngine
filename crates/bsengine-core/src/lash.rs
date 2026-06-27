use bevy_ecs::prelude::Component;

/// Tether or whip attack that pulls the target toward the attacker over time.
///
/// When a lash connects, call `connect(duration)` to begin the pull. Each frame
/// the physics system reads `pull_force` and applies it as an impulse toward the
/// attacker's position. `tick(dt)` counts down the tether duration and sets
/// `just_released` when it ends.
///
/// `damage` stores the hit damage value for systems that need it at connection
/// time but can be left at 0.0 when the caller applies damage separately.
///
/// Distinct from `Knockback` (pushes away) and `Entangle` (roots in place):
/// Lash actively drags the target toward a point over time.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Lash {
    /// Drag force (world units / second²) applied toward the attacker each frame.
    pub pull_force: f32,
    /// Damage dealt on connection (optional; 0.0 if applied separately).
    pub damage: f32,
    /// Duration of the pull in seconds.
    pub duration: f32,
    pub timer: f32,
    pub just_connected: bool,
    pub just_released: bool,
    pub enabled: bool,
}

impl Lash {
    pub fn new(pull_force: f32, damage: f32) -> Self {
        Self {
            pull_force: pull_force.max(0.0),
            damage: damage.max(0.0),
            duration: 0.0,
            timer: 0.0,
            just_connected: false,
            just_released: false,
            enabled: true,
        }
    }

    /// Begin pulling the target. No-op if already connected or disabled.
    pub fn connect(&mut self, duration: f32) {
        if !self.enabled || self.is_connected() {
            return;
        }
        self.duration = duration.max(0.0);
        self.timer = self.duration;
        self.just_connected = true;
    }

    /// Release the tether early.
    pub fn release(&mut self) {
        if self.is_connected() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_released = true;
        }
    }

    /// Advance the timer; sets `just_released` when the pull ends.
    pub fn tick(&mut self, dt: f32) {
        self.just_connected = false;
        self.just_released = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_released = true;
            }
        }
    }

    pub fn is_connected(&self) -> bool {
        self.timer > 0.0
    }

    /// Impulse to apply toward the attacker this frame (`pull_force * dt`).
    /// Returns `0.0` when not connected or disabled.
    pub fn pull_impulse(&self, dt: f32) -> f32 {
        if self.enabled && self.is_connected() {
            self.pull_force * dt
        } else {
            0.0
        }
    }

    /// Fraction of the pull duration remaining [1.0 = just connected, 0.0 = released].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Lash {
    fn default() -> Self {
        Self::new(20.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_starts_pull() {
        let mut l = Lash::new(20.0, 5.0);
        l.connect(2.0);
        assert!(l.is_connected());
        assert!(l.just_connected);
        assert!((l.timer - 2.0).abs() < 1e-5);
    }

    #[test]
    fn connect_no_op_when_already_connected() {
        let mut l = Lash::new(20.0, 5.0);
        l.connect(2.0);
        l.tick(0.016);
        let before = l.timer;
        l.connect(5.0); // should not reset
        assert!((l.timer - before).abs() < 1e-4);
    }

    #[test]
    fn release_ends_pull() {
        let mut l = Lash::new(20.0, 5.0);
        l.connect(5.0);
        l.release();
        assert!(!l.is_connected());
        assert!(l.just_released);
    }

    #[test]
    fn tick_expires_pull() {
        let mut l = Lash::new(20.0, 5.0);
        l.connect(1.0);
        l.tick(1.1);
        assert!(!l.is_connected());
        assert!(l.just_released);
    }

    #[test]
    fn tick_clears_just_connected() {
        let mut l = Lash::new(20.0, 5.0);
        l.connect(2.0);
        l.tick(0.016);
        assert!(!l.just_connected);
    }

    #[test]
    fn pull_impulse_while_connected() {
        let mut l = Lash::new(20.0, 5.0);
        l.connect(3.0);
        let impulse = l.pull_impulse(0.1);
        assert!((impulse - 2.0).abs() < 1e-5); // 20 * 0.1
    }

    #[test]
    fn pull_impulse_when_not_connected() {
        let l = Lash::new(20.0, 5.0);
        assert!((l.pull_impulse(0.1) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut l = Lash::new(20.0, 5.0);
        l.connect(2.0);
        l.tick(1.0);
        assert!((l.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_connect_no_op() {
        let mut l = Lash::new(20.0, 5.0);
        l.enabled = false;
        l.connect(3.0);
        assert!(!l.is_connected());
    }

    #[test]
    fn disabled_pull_impulse_zero() {
        let mut l = Lash::new(20.0, 5.0);
        l.connect(3.0);
        l.enabled = false;
        assert!((l.pull_impulse(0.1) - 0.0).abs() < 1e-5);
    }
}
