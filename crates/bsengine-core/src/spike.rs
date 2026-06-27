use bevy_ecs::prelude::Component;

/// Retractable spike trap that deals damage and pushes back any entity that
/// makes contact while the spikes are extended.
///
/// Call `extend(duration)` to deploy the spikes for a set time; they retract
/// automatically when the timer runs out, or immediately via `retract()`.
/// Contact resolution is the caller's responsibility: read `is_extended()`,
/// deal `damage` to the contact entity, and apply `push_force` as an outward
/// impulse. `tick(dt)` advances the timer and sets `just_retracted` on expiry.
///
/// Distinct from `Thorns` (passive reflect on hit), `Trap` (single-trigger
/// mechanic), and `Knockback` (directional push with no damage over time):
/// Spike is a toggled hazard zone — extended / retracted states, continuous
/// damage on contact while extended.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Spike {
    pub duration: f32,
    pub timer: f32,
    /// Damage dealt per contact event while extended. Clamped ≥ 0.0.
    pub damage: f32,
    /// Outward push force (units/second) applied on contact. Clamped ≥ 0.0.
    pub push_force: f32,
    pub just_extended: bool,
    pub just_retracted: bool,
    pub enabled: bool,
}

impl Spike {
    pub fn new(damage: f32, push_force: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            damage: damage.max(0.0),
            push_force: push_force.max(0.0),
            just_extended: false,
            just_retracted: false,
            enabled: true,
        }
    }

    /// Extend the spikes for `duration` seconds. No-op if already extended or
    /// disabled.
    pub fn extend(&mut self, duration: f32) {
        if !self.enabled || self.is_extended() {
            return;
        }
        self.duration = duration.max(0.0);
        self.timer = self.duration;
        self.just_extended = true;
    }

    /// Retract the spikes immediately.
    pub fn retract(&mut self) {
        if self.is_extended() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_retracted = true;
        }
    }

    /// Advance the timer; sets `just_retracted` when the spike duration expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_extended = false;
        self.just_retracted = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_retracted = true;
            }
        }
    }

    pub fn is_extended(&self) -> bool {
        self.timer > 0.0
    }

    /// Push impulse to apply to a contact entity this frame (`push_force * dt`).
    /// Returns `0.0` when retracted or disabled.
    pub fn push_impulse(&self, dt: f32) -> f32 {
        if self.enabled && self.is_extended() {
            self.push_force * dt
        } else {
            0.0
        }
    }

    /// Fraction of the extension duration remaining [1.0 = just extended, 0.0 = retracted].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Spike {
    fn default() -> Self {
        Self::new(15.0, 10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extend_activates_spike() {
        let mut s = Spike::new(15.0, 10.0);
        s.extend(2.0);
        assert!(s.is_extended());
        assert!(s.just_extended);
    }

    #[test]
    fn extend_no_op_when_already_extended() {
        let mut s = Spike::new(15.0, 10.0);
        s.extend(2.0);
        s.tick(0.016);
        let before = s.timer;
        s.extend(5.0); // should not reset
        assert!((s.timer - before).abs() < 1e-4);
    }

    #[test]
    fn retract_ends_early() {
        let mut s = Spike::new(15.0, 10.0);
        s.extend(5.0);
        s.retract();
        assert!(!s.is_extended());
        assert!(s.just_retracted);
    }

    #[test]
    fn tick_expires_extension() {
        let mut s = Spike::new(15.0, 10.0);
        s.extend(1.0);
        s.tick(1.1);
        assert!(!s.is_extended());
        assert!(s.just_retracted);
    }

    #[test]
    fn tick_clears_just_extended() {
        let mut s = Spike::new(15.0, 10.0);
        s.extend(2.0);
        s.tick(0.016);
        assert!(!s.just_extended);
    }

    #[test]
    fn push_impulse_while_extended() {
        let mut s = Spike::new(15.0, 10.0);
        s.extend(3.0);
        assert!((s.push_impulse(0.1) - 1.0).abs() < 1e-5); // 10 * 0.1
    }

    #[test]
    fn push_impulse_when_retracted() {
        let s = Spike::new(15.0, 10.0);
        assert!((s.push_impulse(0.1) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn push_impulse_disabled() {
        let mut s = Spike::new(15.0, 10.0);
        s.extend(3.0);
        s.enabled = false;
        assert!((s.push_impulse(0.1) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut s = Spike::new(15.0, 10.0);
        s.extend(2.0);
        s.tick(1.0);
        assert!((s.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_extend_no_op() {
        let mut s = Spike::new(15.0, 10.0);
        s.enabled = false;
        s.extend(5.0);
        assert!(!s.is_extended());
    }

    #[test]
    fn negative_params_clamped_to_zero() {
        let s = Spike::new(-5.0, -10.0);
        assert!((s.damage - 0.0).abs() < 1e-5);
        assert!((s.push_force - 0.0).abs() < 1e-5);
    }

    #[test]
    fn can_reextend_after_retract() {
        let mut s = Spike::new(15.0, 10.0);
        s.extend(1.0);
        s.retract();
        s.tick(0.016);
        s.extend(2.0);
        assert!(s.is_extended());
    }
}
