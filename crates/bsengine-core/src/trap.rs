use bevy_ecs::prelude::Component;

/// What the trap does when triggered.
#[derive(Debug, Clone, PartialEq)]
pub enum TrapEffect {
    Damage { amount: f32 },
    Slow { factor: f32, duration: f32 },
    Stun { duration: f32 },
    Custom(String),
}

/// Arming state of the trap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapState {
    /// Waiting for an entity to step on it.
    Armed,
    /// Triggered this frame — physics/gameplay system should apply effects.
    Triggered,
    /// Trap has fired and is cooling down before reset.
    Spent,
    /// Trap has been disarmed and will not fire.
    Disarmed,
}

/// A placed trap entity that fires once (or repeatedly) when an entity
/// enters its trigger volume.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Trap {
    pub state: TrapState,
    pub effect: TrapEffect,
    /// Detection radius in metres.
    pub radius: f32,
    /// Layer mask that controls which entity groups can trigger this trap.
    pub trigger_layer: u32,
    /// Whether the trap resets after firing.
    pub reusable: bool,
    /// Cooldown before a reusable trap rearms (seconds).
    pub rearm_time: f32,
    pub rearm_accumulated: f32,
    pub enabled: bool,
}

impl Trap {
    pub fn new(effect: TrapEffect, radius: f32) -> Self {
        Self {
            state: TrapState::Armed,
            effect,
            radius: radius.max(0.0),
            trigger_layer: u32::MAX,
            reusable: false,
            rearm_time: 5.0,
            rearm_accumulated: 0.0,
            enabled: true,
        }
    }

    pub fn damage(amount: f32, radius: f32) -> Self {
        Self::new(TrapEffect::Damage { amount }, radius)
    }

    pub fn slow(factor: f32, duration: f32, radius: f32) -> Self {
        Self::new(TrapEffect::Slow { factor, duration }, radius)
    }

    pub fn reusable(mut self) -> Self {
        self.reusable = true;
        self
    }

    pub fn with_rearm_time(mut self, seconds: f32) -> Self {
        self.rearm_time = seconds.max(0.0);
        self
    }

    pub fn with_layer(mut self, layer: u32) -> Self {
        self.trigger_layer = layer;
        self
    }

    pub fn disarmed(mut self) -> Self {
        self.state = TrapState::Disarmed;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Fire the trap. Returns `false` if not armed or disabled.
    pub fn trigger(&mut self) -> bool {
        if !self.enabled || self.state != TrapState::Armed {
            return false;
        }
        self.state = TrapState::Triggered;
        true
    }

    /// Call after the effect has been applied this frame.
    pub fn consume(&mut self) {
        self.state = if self.reusable {
            self.rearm_accumulated = 0.0;
            TrapState::Spent
        } else {
            TrapState::Disarmed
        };
    }

    /// Advance rearm timer. Returns `true` when a reusable trap rearms.
    pub fn tick(&mut self, dt: f32) -> bool {
        if self.state == TrapState::Spent {
            self.rearm_accumulated += dt;
            if self.rearm_accumulated >= self.rearm_time {
                self.rearm_accumulated = 0.0;
                self.state = TrapState::Armed;
                return true;
            }
        }
        false
    }

    pub fn is_armed(&self) -> bool {
        self.enabled && self.state == TrapState::Armed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trap_trigger_sets_triggered() {
        let mut t = Trap::damage(50.0, 1.0);
        assert!(t.trigger());
        assert_eq!(t.state, TrapState::Triggered);
    }

    #[test]
    fn trap_single_use_disarms_on_consume() {
        let mut t = Trap::damage(10.0, 0.5);
        t.trigger();
        t.consume();
        assert_eq!(t.state, TrapState::Disarmed);
    }

    #[test]
    fn trap_reusable_rearms_after_cooldown() {
        let mut t = Trap::damage(10.0, 0.5).reusable().with_rearm_time(1.0);
        t.trigger();
        t.consume();
        assert!(!t.tick(0.5));
        assert!(t.tick(0.6));
        assert!(t.is_armed());
    }

    #[test]
    fn trap_disarmed_rejects_trigger() {
        let mut t = Trap::damage(5.0, 0.5).disarmed();
        assert!(!t.trigger());
    }

    #[test]
    fn trap_disabled_rejects_trigger() {
        let mut t = Trap::slow(0.5, 2.0, 1.0).disabled();
        assert!(!t.trigger());
    }
}
