use bevy_ecs::prelude::Component;

/// Radiant fire aura that deals heat damage to all entities within `radius`
/// each frame while active.
///
/// When blazing, the damage pipeline reads `tick(dt)` which returns the damage
/// pulse for this frame (`damage_per_second * dt`). Detection systems that
/// handle proximity damage apply this value to every entity within `radius`.
///
/// `ignite(duration)` starts the blaze. No-op if already blazing or disabled.
/// `extinguish()` stops the aura early. `tick(dt)` counts down and sets
/// `just_extinguished` on expiry.
///
/// Distinct from `Burn` (single-target DoT tick applied to the entity itself)
/// and `Ignite` (charge accumulator that triggers a burn threshold): Blaze is
/// an outward radiating aura damaging nearby enemies, not a debuff on self.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Blaze {
    /// World-unit radius of the heat aura.
    pub radius: f32,
    /// Damage per second dealt to each entity inside `radius`.
    pub damage_per_second: f32,
    pub duration: f32,
    pub timer: f32,
    pub just_blazing: bool,
    pub just_extinguished: bool,
    pub enabled: bool,
}

impl Blaze {
    pub fn new(radius: f32, damage_per_second: f32) -> Self {
        Self {
            radius: radius.max(0.0),
            damage_per_second: damage_per_second.max(0.0),
            duration: 0.0,
            timer: 0.0,
            just_blazing: false,
            just_extinguished: false,
            enabled: true,
        }
    }

    /// Start the blaze for `duration` seconds. No-op if already blazing or disabled.
    pub fn ignite(&mut self, duration: f32) {
        if !self.enabled || self.is_blazing() {
            return;
        }
        self.duration = duration.max(0.0);
        self.timer = self.duration;
        self.just_blazing = true;
    }

    /// Stop the aura immediately.
    pub fn extinguish(&mut self) {
        if self.is_blazing() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_extinguished = true;
        }
    }

    /// Advance the timer. Returns the damage pulse for this frame
    /// (`damage_per_second * dt`); 0.0 when not blazing.
    /// Sets `just_extinguished` when the duration runs out.
    pub fn tick(&mut self, dt: f32) -> f32 {
        self.just_blazing = false;
        self.just_extinguished = false;

        if self.timer <= 0.0 {
            return 0.0;
        }

        self.timer -= dt;
        if self.timer <= 0.0 {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_extinguished = true;
            return 0.0;
        }

        self.damage_per_second * dt
    }

    pub fn is_blazing(&self) -> bool {
        self.timer > 0.0
    }

    /// Fraction of the blaze duration remaining [1.0 = just ignited, 0.0 = out].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Blaze {
    fn default() -> Self {
        Self::new(3.0, 15.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignite_starts_blaze() {
        let mut b = Blaze::new(3.0, 10.0);
        b.ignite(2.0);
        assert!(b.is_blazing());
        assert!(b.just_blazing);
    }

    #[test]
    fn ignite_no_op_when_already_blazing() {
        let mut b = Blaze::new(3.0, 10.0);
        b.ignite(2.0);
        b.tick(0.016);
        let before = b.timer;
        b.ignite(5.0);
        assert!((b.timer - before).abs() < 1e-4);
    }

    #[test]
    fn extinguish_stops_blaze() {
        let mut b = Blaze::new(3.0, 10.0);
        b.ignite(5.0);
        b.extinguish();
        assert!(!b.is_blazing());
        assert!(b.just_extinguished);
    }

    #[test]
    fn tick_returns_damage_pulse() {
        let mut b = Blaze::new(3.0, 10.0);
        b.ignite(5.0);
        let damage = b.tick(0.1);
        assert!((damage - 1.0).abs() < 1e-5); // 10 * 0.1
    }

    #[test]
    fn tick_returns_zero_when_inactive() {
        let mut b = Blaze::new(3.0, 10.0);
        let damage = b.tick(0.1);
        assert!((damage - 0.0).abs() < 1e-5);
    }

    #[test]
    fn tick_expires_blaze() {
        let mut b = Blaze::new(3.0, 10.0);
        b.ignite(1.0);
        b.tick(1.1);
        assert!(!b.is_blazing());
        assert!(b.just_extinguished);
    }

    #[test]
    fn tick_returns_zero_on_expiry_frame() {
        let mut b = Blaze::new(3.0, 10.0);
        b.ignite(0.05);
        let damage = b.tick(0.1);
        assert!((damage - 0.0).abs() < 1e-5);
        assert!(b.just_extinguished);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut b = Blaze::new(3.0, 10.0);
        b.ignite(2.0);
        b.tick(1.0);
        assert!((b.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_ignite_no_op() {
        let mut b = Blaze::new(3.0, 10.0);
        b.enabled = false;
        b.ignite(5.0);
        assert!(!b.is_blazing());
    }

    #[test]
    fn tick_clears_just_blazing() {
        let mut b = Blaze::new(3.0, 10.0);
        b.ignite(3.0);
        b.tick(0.016);
        assert!(!b.just_blazing);
    }
}
