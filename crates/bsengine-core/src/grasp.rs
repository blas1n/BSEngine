use bevy_ecs::prelude::Component;

/// Timed hold attack: the entity seizes a target, dealing squeeze damage each
/// frame and preventing movement for the duration (when `suppress_movement` is
/// true).
///
/// Call `start(duration)` to begin the hold. `tick(dt)` counts down and
/// returns `damage_per_second * dt` as squeeze damage each frame; it also sets
/// `just_released` when the hold expires. `release()` ends the hold early.
///
/// Distinct from `Grapple` (two-body wrestling physics), `Entangle` (vines/
/// chains root), and `Lash` (pull toward attacker): Grasp is a one-body hold —
/// _this_ entity's hands close around the target; the target entity would
/// carry a separate `Grasped` marker (outside this component's scope).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Grasp {
    pub duration: f32,
    pub timer: f32,
    /// Squeeze damage dealt to the held target per second.
    pub damage_per_second: f32,
    /// Whether the grasping entity's movement is also suppressed while holding.
    pub suppress_movement: bool,
    pub just_grasped: bool,
    pub just_released: bool,
    pub enabled: bool,
}

impl Grasp {
    pub fn new(damage_per_second: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            damage_per_second: damage_per_second.max(0.0),
            suppress_movement: true,
            just_grasped: false,
            just_released: false,
            enabled: true,
        }
    }

    pub fn with_suppress_movement(mut self, suppress: bool) -> Self {
        self.suppress_movement = suppress;
        self
    }

    /// Begin the grasp for `duration` seconds. No-op when disabled or already
    /// holding.
    pub fn start(&mut self, duration: f32) {
        if !self.enabled || self.is_active() {
            return;
        }
        self.duration = duration.max(0.0);
        self.timer = self.duration;
        self.just_grasped = true;
    }

    /// Release the hold early.
    pub fn release(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_released = true;
        }
    }

    /// Advance the timer; returns squeeze damage for this frame
    /// (`damage_per_second * dt`). Sets `just_released` when the hold expires.
    pub fn tick(&mut self, dt: f32) -> f32 {
        self.just_grasped = false;
        self.just_released = false;

        if self.timer > 0.0 {
            let damage = self.damage_per_second * dt;
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_released = true;
            }
            return damage;
        }
        0.0
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Fraction of the grasp duration remaining [1.0 = just started, 0.0 = released].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Grasp {
    fn default() -> Self {
        Self::new(10.0).with_suppress_movement(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_activates_grasp() {
        let mut g = Grasp::new(10.0);
        g.start(3.0);
        assert!(g.is_active());
        assert!(g.just_grasped);
    }

    #[test]
    fn start_no_op_when_already_active() {
        let mut g = Grasp::new(10.0);
        g.start(3.0);
        g.tick(0.016);
        let timer_before = g.timer;
        g.start(5.0); // should not reset
        assert!((g.timer - timer_before).abs() < 1e-4);
    }

    #[test]
    fn release_ends_early() {
        let mut g = Grasp::new(10.0);
        g.start(5.0);
        g.release();
        assert!(!g.is_active());
        assert!(g.just_released);
    }

    #[test]
    fn tick_returns_squeeze_damage() {
        let mut g = Grasp::new(20.0);
        g.start(5.0);
        let dmg = g.tick(0.5);
        assert!((dmg - 10.0).abs() < 1e-4); // 20 * 0.5
    }

    #[test]
    fn tick_zero_damage_when_inactive() {
        let mut g = Grasp::new(20.0);
        let dmg = g.tick(1.0);
        assert!((dmg - 0.0).abs() < 1e-5);
    }

    #[test]
    fn tick_expires_grasp() {
        let mut g = Grasp::new(10.0);
        g.start(1.0);
        g.tick(1.1);
        assert!(!g.is_active());
        assert!(g.just_released);
    }

    #[test]
    fn tick_clears_just_grasped() {
        let mut g = Grasp::new(10.0);
        g.start(3.0);
        g.tick(0.016);
        assert!(!g.just_grasped);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut g = Grasp::new(10.0);
        g.start(2.0);
        g.tick(1.0);
        assert!((g.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_start_no_op() {
        let mut g = Grasp::new(10.0);
        g.enabled = false;
        g.start(3.0);
        assert!(!g.is_active());
    }

    #[test]
    fn suppress_movement_default_true() {
        let g = Grasp::new(10.0);
        assert!(g.suppress_movement);
    }

    #[test]
    fn with_suppress_movement_false() {
        let g = Grasp::new(10.0).with_suppress_movement(false);
        assert!(!g.suppress_movement);
    }
}
