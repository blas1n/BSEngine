use bevy_ecs::prelude::Component;

/// Momentum-scaled melee strike: the faster the entity is moving when it
/// thrusts, the more damage the lance delivers.
///
/// Call `thrust(current_speed, duration)` to begin the strike (no-op if
/// already striking, below threshold, or disabled). While `is_striking()`,
/// the collision system reads `strike_damage(current_speed)` to compute
/// impact damage against any entity hit by the lance hitbox. `tick(dt)` counts
/// down the active duration and sets `just_ended` on expiry. `retract()`
/// ends the strike early.
///
/// Distinct from `Charge` (self-locomotion that moves the attacker forward),
/// `Melee` (general swing, no speed scaling), and `Pierce` (projectile
/// penetration through multiple targets): Lance is a **stationary** held-out
/// hitbox whose damage scales with the attacker's velocity — ideal for
/// mounted charges, dash-attacks, or run-through strikes.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Lance {
    pub duration: f32,
    pub timer: f32,
    /// Base damage at the minimum speed threshold.
    pub base_damage: f32,
    /// Additional damage per world-unit/s of speed above `speed_threshold`.
    /// Clamped ≥ 0.0.
    pub speed_scale: f32,
    /// Minimum speed (world-units/s) required to trigger a thrust. Clamped ≥ 0.0.
    pub speed_threshold: f32,
    pub just_struck: bool,
    pub just_ended: bool,
    pub enabled: bool,
}

impl Lance {
    pub fn new(base_damage: f32, speed_scale: f32, speed_threshold: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            base_damage: base_damage.max(0.0),
            speed_scale: speed_scale.max(0.0),
            speed_threshold: speed_threshold.max(0.0),
            just_struck: false,
            just_ended: false,
            enabled: true,
        }
    }

    /// Begin the lance strike for `duration` seconds at `current_speed`.
    /// No-op when already striking, below `speed_threshold`, or disabled.
    pub fn thrust(&mut self, current_speed: f32, duration: f32) {
        if !self.enabled || self.is_striking() || current_speed < self.speed_threshold {
            return;
        }
        self.duration = duration.max(0.0);
        self.timer = self.duration;
        self.just_struck = true;
    }

    /// End the lance strike early (e.g., attacker stops moving or is interrupted).
    pub fn retract(&mut self) {
        if self.is_striking() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_ended = true;
        }
    }

    /// Advance the timer; sets `just_ended` when the strike duration expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_struck = false;
        self.just_ended = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_ended = true;
            }
        }
    }

    pub fn is_striking(&self) -> bool {
        self.timer > 0.0
    }

    /// Whether `current_speed` meets the thrust threshold.
    pub fn meets_threshold(&self, current_speed: f32) -> bool {
        current_speed >= self.speed_threshold
    }

    /// Damage dealt on impact at `current_speed`.
    /// Returns `base_damage + (speed - threshold).max(0) * speed_scale`
    /// while striking and enabled. Returns `0.0` otherwise.
    pub fn strike_damage(&self, current_speed: f32) -> f32 {
        if !self.is_striking() || !self.enabled {
            return 0.0;
        }
        let bonus = (current_speed - self.speed_threshold).max(0.0) * self.speed_scale;
        self.base_damage + bonus
    }

    /// Fraction of the strike duration remaining [1.0 = just thrust, 0.0 = ended].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Lance {
    fn default() -> Self {
        Self::new(30.0, 2.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thrust_starts_strike() {
        let mut l = Lance::new(30.0, 2.0, 5.0);
        l.thrust(8.0, 0.3);
        assert!(l.is_striking());
        assert!(l.just_struck);
    }

    #[test]
    fn thrust_no_op_below_threshold() {
        let mut l = Lance::new(30.0, 2.0, 5.0);
        l.thrust(3.0, 0.3); // below 5.0
        assert!(!l.is_striking());
    }

    #[test]
    fn thrust_no_op_when_already_striking() {
        let mut l = Lance::new(30.0, 2.0, 5.0);
        l.thrust(8.0, 0.3);
        l.tick(0.016);
        let before = l.timer;
        l.thrust(10.0, 1.0); // should not reset
        assert!((l.timer - before).abs() < 1e-4);
    }

    #[test]
    fn retract_ends_strike() {
        let mut l = Lance::new(30.0, 2.0, 5.0);
        l.thrust(8.0, 0.5);
        l.retract();
        assert!(!l.is_striking());
        assert!(l.just_ended);
    }

    #[test]
    fn tick_expires_strike() {
        let mut l = Lance::new(30.0, 2.0, 5.0);
        l.thrust(8.0, 0.3);
        l.tick(0.5);
        assert!(!l.is_striking());
        assert!(l.just_ended);
    }

    #[test]
    fn tick_clears_just_struck() {
        let mut l = Lance::new(30.0, 2.0, 5.0);
        l.thrust(8.0, 0.5);
        l.tick(0.016);
        assert!(!l.just_struck);
    }

    #[test]
    fn strike_damage_at_threshold() {
        let mut l = Lance::new(30.0, 2.0, 5.0);
        l.thrust(5.0, 0.5); // exactly at threshold
        let dmg = l.strike_damage(5.0);
        assert!((dmg - 30.0).abs() < 1e-4); // base only
    }

    #[test]
    fn strike_damage_scales_with_speed() {
        let mut l = Lance::new(30.0, 2.0, 5.0);
        l.thrust(10.0, 0.5);
        let dmg = l.strike_damage(10.0);
        // 30 + (10-5)*2 = 40
        assert!((dmg - 40.0).abs() < 1e-4);
    }

    #[test]
    fn strike_damage_zero_when_not_striking() {
        let l = Lance::new(30.0, 2.0, 5.0);
        assert!((l.strike_damage(20.0) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn meets_threshold_correct() {
        let l = Lance::new(30.0, 2.0, 5.0);
        assert!(l.meets_threshold(5.0));
        assert!(l.meets_threshold(10.0));
        assert!(!l.meets_threshold(4.9));
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut l = Lance::new(30.0, 2.0, 5.0);
        l.thrust(8.0, 1.0);
        l.tick(0.5);
        assert!((l.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_thrust_no_op() {
        let mut l = Lance::new(30.0, 2.0, 5.0);
        l.enabled = false;
        l.thrust(10.0, 0.5);
        assert!(!l.is_striking());
    }

    #[test]
    fn disabled_strike_damage_zero() {
        let mut l = Lance::new(30.0, 2.0, 5.0);
        l.thrust(10.0, 0.5);
        l.enabled = false;
        assert!((l.strike_damage(10.0) - 0.0).abs() < 1e-5);
    }
}
