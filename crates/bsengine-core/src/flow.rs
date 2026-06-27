use bevy_ecs::prelude::Component;

/// "In-the-zone" buff that reduces ability cooldowns and increases action speed.
///
/// While in flow, cooldown systems should pass their remaining time through
/// `effective_cooldown(base)` to apply the reduction, and attack/cast-speed
/// systems should use `effective_action_speed(base)` for the speed bonus.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_exited` on expiry.
///
/// Distinct from `Haste` (movement speed only), `Amplify` (flat damage
/// multiplier), and `Empower` (damage output boost): Flow is a peak-efficiency
/// buff — it makes actions happen faster and cooldowns shorter, representing an
/// entity performing at the height of their reflexes and muscle memory.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Flow {
    pub duration: f32,
    pub timer: f32,
    /// Fraction [0.0, 1.0] by which cooldown times are shortened while in flow.
    /// e.g. 0.3 = cooldowns take 30% less time.
    pub cooldown_reduction_fraction: f32,
    /// Fractional bonus to action speed (attack/cast rate) while in flow.
    /// e.g. 0.2 = actions execute 20% faster.
    pub action_speed_bonus: f32,
    pub just_entered: bool,
    pub just_exited: bool,
    pub enabled: bool,
}

impl Flow {
    pub fn new(cooldown_reduction_fraction: f32, action_speed_bonus: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            cooldown_reduction_fraction: cooldown_reduction_fraction.clamp(0.0, 1.0),
            action_speed_bonus: action_speed_bonus.max(0.0),
            just_entered: false,
            just_exited: false,
            enabled: true,
        }
    }

    /// Apply or extend the flow buff for `duration` seconds. High-watermark:
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
                self.just_entered = true;
            }
        }
    }

    /// Break flow immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_exited = true;
        }
    }

    /// Advance the timer; sets `just_exited` when the buff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_entered = false;
        self.just_exited = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_exited = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective cooldown with flow reduction. Returns `base * (1 - fraction)`
    /// while active, `base` otherwise.
    pub fn effective_cooldown(&self, base: f32) -> f32 {
        if self.is_active() {
            (base * (1.0 - self.cooldown_reduction_fraction)).max(0.0)
        } else {
            base
        }
    }

    /// Effective action speed with flow bonus. Returns `base * (1 + bonus)`
    /// while active, `base` otherwise.
    pub fn effective_action_speed(&self, base: f32) -> f32 {
        if self.is_active() {
            base * (1.0 + self.action_speed_bonus)
        } else {
            base
        }
    }

    /// Fraction of the flow duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Flow {
    fn default() -> Self {
        Self::new(0.3, 0.2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_flow() {
        let mut f = Flow::new(0.3, 0.2);
        f.apply(3.0);
        assert!(f.is_active());
        assert!(f.just_entered);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut f = Flow::new(0.3, 0.2);
        f.apply(2.0);
        f.tick(0.016);
        f.apply(5.0);
        assert!((f.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut f = Flow::new(0.3, 0.2);
        f.apply(5.0);
        f.apply(2.0);
        assert!((f.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_flow() {
        let mut f = Flow::new(0.3, 0.2);
        f.apply(1.0);
        f.tick(1.1);
        assert!(!f.is_active());
        assert!(f.just_exited);
    }

    #[test]
    fn clear_ends_early() {
        let mut f = Flow::new(0.3, 0.2);
        f.apply(5.0);
        f.clear();
        assert!(!f.is_active());
        assert!(f.just_exited);
    }

    #[test]
    fn effective_cooldown_while_active() {
        let mut f = Flow::new(0.3, 0.2);
        f.apply(3.0);
        assert!((f.effective_cooldown(10.0) - 7.0).abs() < 1e-4); // 10 * 0.7
    }

    #[test]
    fn effective_cooldown_when_inactive() {
        let f = Flow::new(0.3, 0.2);
        assert!((f.effective_cooldown(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn effective_action_speed_while_active() {
        let mut f = Flow::new(0.3, 0.2);
        f.apply(3.0);
        assert!((f.effective_action_speed(10.0) - 12.0).abs() < 1e-4); // 10 * 1.2
    }

    #[test]
    fn effective_action_speed_when_inactive() {
        let f = Flow::new(0.3, 0.2);
        assert!((f.effective_action_speed(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut f = Flow::new(0.3, 0.2);
        f.apply(2.0);
        f.tick(1.0);
        assert!((f.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut f = Flow::new(0.3, 0.2);
        f.enabled = false;
        f.apply(5.0);
        assert!(!f.is_active());
    }

    #[test]
    fn tick_clears_just_entered() {
        let mut f = Flow::new(0.3, 0.2);
        f.apply(3.0);
        f.tick(0.016);
        assert!(!f.just_entered);
    }

    #[test]
    fn cooldown_reduction_clamped() {
        let f = Flow::new(1.5, 0.2);
        assert!((f.cooldown_reduction_fraction - 1.0).abs() < 1e-5);
        // 100% reduction: cooldown becomes 0
        let mut f2 = Flow::new(1.0, 0.2);
        f2.apply(3.0);
        assert!((f2.effective_cooldown(10.0) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn action_speed_bonus_clamped_to_zero() {
        let f = Flow::new(0.3, -0.5);
        assert!((f.action_speed_bonus - 0.0).abs() < 1e-5);
    }
}
