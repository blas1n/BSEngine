use bevy_ecs::prelude::Component;

/// Momentum-based charge state that lets an entity bulldoze through enemies,
/// dealing impact damage and pushing them aside on contact.
///
/// While trampling, physics/combat systems should detect enemies within the
/// entity's forward path and, per frame or per contact:
/// - Apply `damage` as physical damage to the collided entity.
/// - Apply `push_force` as an impulse away from this entity's center.
///
/// `start(duration)` begins the trample (no-op if already active or disabled).
/// `stop()` ends it early. `tick(dt)` counts down and sets `just_ended` on
/// expiry.
///
/// Distinct from `Dash` (single burst of movement), `Lunge` (single targeted
/// attack lunge), and `Charge` (windup + release attack phase): Trample is an
/// ongoing ground-pound locomotion state — the entity is continuously moving
/// and dealing damage across its whole path for the duration.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Trample {
    pub duration: f32,
    pub timer: f32,
    /// Physical damage dealt to each enemy contacted per trample tick.
    pub damage: f32,
    /// Impulse force applied outward to each contacted enemy.
    pub push_force: f32,
    /// Radius around the entity's position that counts as a trample collision.
    pub radius: f32,
    pub just_started: bool,
    pub just_ended: bool,
    pub enabled: bool,
}

impl Trample {
    pub fn new(damage: f32, push_force: f32, radius: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            damage: damage.max(0.0),
            push_force: push_force.max(0.0),
            radius: radius.max(0.0),
            just_started: false,
            just_ended: false,
            enabled: true,
        }
    }

    /// Begin trampling for `duration` seconds. No-op if already active or disabled.
    pub fn start(&mut self, duration: f32) {
        if !self.enabled || self.is_active() {
            return;
        }
        self.duration = duration.max(0.0);
        self.timer = self.duration;
        self.just_started = true;
    }

    /// End the trample immediately.
    pub fn stop(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_ended = true;
        }
    }

    /// Advance the timer; sets `just_ended` when the trample expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_started = false;
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

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Fraction of the trample duration remaining [1.0 = just started, 0.0 = ended].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Trample {
    fn default() -> Self {
        Self::new(30.0, 15.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_activates_trample() {
        let mut t = Trample::new(30.0, 15.0, 2.0);
        t.start(2.0);
        assert!(t.is_active());
        assert!(t.just_started);
    }

    #[test]
    fn start_no_op_when_already_active() {
        let mut t = Trample::new(30.0, 15.0, 2.0);
        t.start(2.0);
        t.tick(0.016);
        let before = t.timer;
        t.start(5.0);
        assert!((t.timer - before).abs() < 1e-4);
    }

    #[test]
    fn stop_ends_trample() {
        let mut t = Trample::new(30.0, 15.0, 2.0);
        t.start(5.0);
        t.stop();
        assert!(!t.is_active());
        assert!(t.just_ended);
    }

    #[test]
    fn tick_expires_trample() {
        let mut t = Trample::new(30.0, 15.0, 2.0);
        t.start(1.0);
        t.tick(1.1);
        assert!(!t.is_active());
        assert!(t.just_ended);
    }

    #[test]
    fn tick_clears_just_started() {
        let mut t = Trample::new(30.0, 15.0, 2.0);
        t.start(2.0);
        t.tick(0.016);
        assert!(!t.just_started);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut t = Trample::new(30.0, 15.0, 2.0);
        t.start(2.0);
        t.tick(1.0);
        assert!((t.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_start_no_op() {
        let mut t = Trample::new(30.0, 15.0, 2.0);
        t.enabled = false;
        t.start(3.0);
        assert!(!t.is_active());
    }

    #[test]
    fn negative_params_clamped_to_zero() {
        let t = Trample::new(-5.0, -10.0, -1.0);
        assert!((t.damage - 0.0).abs() < 1e-5);
        assert!((t.push_force - 0.0).abs() < 1e-5);
        assert!((t.radius - 0.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_zero_before_start() {
        let t = Trample::new(30.0, 15.0, 2.0);
        assert!((t.remaining_fraction() - 0.0).abs() < 1e-5);
    }
}
