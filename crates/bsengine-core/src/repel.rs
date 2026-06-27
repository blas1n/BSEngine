use bevy_ecs::prelude::Component;

/// Radial force aura that continuously pushes nearby enemies away from this
/// entity while active.
///
/// Each frame, the physics system should find all enemy entities within `radius`
/// of this entity and apply an outward impulse equal to `push_impulse(dt)` to
/// each of them. Enemies at the center receive the same force as those at the
/// edge — callers can scale by distance if desired.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_deactivated` when the aura fades.
///
/// Distinct from `Knockback` (single burst push), `Lash` (pulls toward
/// attacker), and `Taunt` (draws enemies closer): Repel is a persistent radial
/// aura that continuously drives enemies back for its duration.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Repel {
    pub duration: f32,
    pub timer: f32,
    /// Continuous outward push force (units per second).
    /// Physics system should apply `push_impulse(dt)` each frame.
    pub push_force: f32,
    /// Radius within which the aura pushes enemies.
    pub radius: f32,
    pub just_activated: bool,
    pub just_deactivated: bool,
    pub enabled: bool,
}

impl Repel {
    pub fn new(push_force: f32, radius: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            push_force: push_force.max(0.0),
            radius: radius.max(0.0),
            just_activated: false,
            just_deactivated: false,
            enabled: true,
        }
    }

    /// Apply or extend the repel aura for `duration` seconds. High-watermark:
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
                self.just_activated = true;
            }
        }
    }

    /// End the aura immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_deactivated = true;
        }
    }

    /// Advance the timer; sets `just_deactivated` when the aura expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_activated = false;
        self.just_deactivated = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_deactivated = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Outward impulse to apply to each enemy this frame (`push_force * dt`).
    /// Returns `0.0` when inactive or disabled.
    pub fn push_impulse(&self, dt: f32) -> f32 {
        if self.is_active() {
            self.push_force * dt
        } else {
            0.0
        }
    }

    /// Fraction of the repel duration remaining [1.0 = just activated, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Repel {
    fn default() -> Self {
        Self::new(20.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_repel() {
        let mut r = Repel::new(20.0, 5.0);
        r.apply(3.0);
        assert!(r.is_active());
        assert!(r.just_activated);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut r = Repel::new(20.0, 5.0);
        r.apply(2.0);
        r.tick(0.016);
        r.apply(5.0);
        assert!((r.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut r = Repel::new(20.0, 5.0);
        r.apply(5.0);
        r.apply(2.0);
        assert!((r.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_repel() {
        let mut r = Repel::new(20.0, 5.0);
        r.apply(1.0);
        r.tick(1.1);
        assert!(!r.is_active());
        assert!(r.just_deactivated);
    }

    #[test]
    fn clear_ends_early() {
        let mut r = Repel::new(20.0, 5.0);
        r.apply(5.0);
        r.clear();
        assert!(!r.is_active());
        assert!(r.just_deactivated);
    }

    #[test]
    fn push_impulse_while_active() {
        let mut r = Repel::new(20.0, 5.0);
        r.apply(3.0);
        assert!((r.push_impulse(0.1) - 2.0).abs() < 1e-5); // 20 * 0.1
    }

    #[test]
    fn push_impulse_when_inactive() {
        let r = Repel::new(20.0, 5.0);
        assert!((r.push_impulse(0.1) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut r = Repel::new(20.0, 5.0);
        r.apply(2.0);
        r.tick(1.0);
        assert!((r.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut r = Repel::new(20.0, 5.0);
        r.enabled = false;
        r.apply(5.0);
        assert!(!r.is_active());
    }

    #[test]
    fn tick_clears_just_activated() {
        let mut r = Repel::new(20.0, 5.0);
        r.apply(3.0);
        r.tick(0.016);
        assert!(!r.just_activated);
    }

    #[test]
    fn negative_params_clamped_to_zero() {
        let r = Repel::new(-5.0, -1.0);
        assert!((r.push_force - 0.0).abs() < 1e-5);
        assert!((r.radius - 0.0).abs() < 1e-5);
    }
}
