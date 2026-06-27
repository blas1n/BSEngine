use bevy_ecs::prelude::Component;

/// Burning / on-fire state component.
///
/// The elemental-damage system calls `ignite()` to start a burn. Each frame
/// `tick(dt)` returns the damage-per-second to apply and updates `intensity`.
/// When `remaining` reaches zero the entity is extinguished automatically.
/// Subsequent `ignite()` calls refresh the duration and stack up to
/// `max_stacks` times, each adding `burn_rate` damage and refreshing the timer.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Burn {
    /// Damage dealt per second per stack.
    pub burn_rate: f32,
    /// Current stack count. Each `ignite()` call increments this up to `max_stacks`.
    pub stacks: u32,
    /// Maximum number of stacks.
    pub max_stacks: u32,
    /// Remaining burn duration (seconds).
    pub remaining: f32,
    /// Total burn duration for a single ignite call (seconds).
    pub duration: f32,
    /// Visual intensity [0.0, 1.0] — fraction of time remaining.
    pub intensity: f32,
    /// True for the frame `ignite()` is called (cleared next `tick()`).
    pub just_ignited: bool,
    /// True when the burn extinguished this frame (cleared next `tick()`).
    pub just_extinguished: bool,
    /// Whether this entity can be set on fire.
    pub ignitable: bool,
    pub enabled: bool,
}

impl Burn {
    pub fn new(burn_rate: f32, duration: f32) -> Self {
        Self {
            burn_rate: burn_rate.max(0.0),
            stacks: 0,
            max_stacks: 3,
            remaining: 0.0,
            duration: duration.max(0.0),
            intensity: 0.0,
            just_ignited: false,
            just_extinguished: false,
            ignitable: true,
            enabled: true,
        }
    }

    pub fn with_max_stacks(mut self, n: u32) -> Self {
        self.max_stacks = n.max(1);
        self
    }

    pub fn not_ignitable(mut self) -> Self {
        self.ignitable = false;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Set the entity on fire (or add a stack). Returns false if not ignitable or disabled.
    pub fn ignite(&mut self) -> bool {
        if !self.enabled || !self.ignitable {
            return false;
        }
        self.stacks = (self.stacks + 1).min(self.max_stacks);
        self.remaining = self.duration; // refresh timer
        self.just_ignited = true;
        true
    }

    /// Immediately extinguish all stacks.
    pub fn extinguish(&mut self) {
        self.stacks = 0;
        self.remaining = 0.0;
        self.intensity = 0.0;
    }

    /// Advance the burn. Returns damage to apply this frame (stacks * burn_rate * dt).
    pub fn tick(&mut self, dt: f32) -> f32 {
        self.just_ignited = false;
        self.just_extinguished = false;

        if !self.enabled || self.stacks == 0 {
            return 0.0;
        }

        self.remaining -= dt;
        if self.remaining <= 0.0 {
            self.remaining = 0.0;
            self.stacks = 0;
            self.intensity = 0.0;
            self.just_extinguished = true;
            return 0.0;
        }

        self.intensity = (self.remaining / self.duration).clamp(0.0, 1.0);
        self.stacks as f32 * self.burn_rate * dt
    }

    pub fn is_burning(&self) -> bool {
        self.enabled && self.stacks > 0 && self.remaining > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignite_sets_burning() {
        let mut b = Burn::new(10.0, 3.0);
        let ok = b.ignite();
        assert!(ok);
        assert!(b.is_burning());
        assert_eq!(b.stacks, 1);
    }

    #[test]
    fn tick_deals_damage() {
        let mut b = Burn::new(10.0, 3.0);
        b.ignite();
        let dmg = b.tick(1.0);
        assert!((dmg - 10.0).abs() < 1e-4);
    }

    #[test]
    fn stacks_multiply_damage() {
        let mut b = Burn::new(10.0, 3.0).with_max_stacks(3);
        b.ignite();
        b.ignite();
        let dmg = b.tick(1.0);
        assert!((dmg - 20.0).abs() < 1e-4); // 2 stacks
    }

    #[test]
    fn tick_extinguishes_when_duration_expires() {
        let mut b = Burn::new(10.0, 0.5);
        b.ignite();
        b.tick(0.6);
        assert!(!b.is_burning());
        assert!(b.just_extinguished);
    }

    #[test]
    fn extinguish_clears_immediately() {
        let mut b = Burn::new(10.0, 3.0);
        b.ignite();
        b.extinguish();
        assert!(!b.is_burning());
        assert_eq!(b.stacks, 0);
    }

    #[test]
    fn not_ignitable_rejects_ignite() {
        let mut b = Burn::new(10.0, 3.0).not_ignitable();
        let ok = b.ignite();
        assert!(!ok);
        assert!(!b.is_burning());
    }
}
