use bevy_ecs::prelude::Component;

/// Which resource the regeneration targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegenTarget {
    /// Restore `health`.
    Health,
    /// Restore `mana`.
    Mana,
    /// Restore `stamina`.
    Stamina,
    /// Generic — caller maps to their own resource.
    Custom,
}

/// Passive resource-regeneration component.
///
/// Regenerates a resource (health, mana, stamina) at `rate` units per second.
/// A damage-interrupt mechanism delays regeneration by `delay_after_damage`
/// seconds whenever `notify_damage()` is called.
///
/// The component does not modify resources directly — it accumulates
/// `pending` units per tick. The owning system drains `pending` and applies
/// it to the appropriate resource each frame.
///
/// Distinct from `health` (raw HP pool), `mana`, and `stamina` (resource pools).
/// `Regen` is the passive top-up layer that belongs alongside those components.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Regen {
    pub target: RegenTarget,
    /// Units regenerated per second.
    pub rate: f32,
    /// Seconds to wait after `notify_damage()` before regen resumes.
    pub delay_after_damage: f32,
    /// Countdown from `delay_after_damage`; regen is suppressed while > 0.
    pub delay_timer: f32,
    /// Accumulated regen this frame; drain and apply to the target resource.
    pub pending: f32,
    pub enabled: bool,
}

impl Regen {
    pub fn new(target: RegenTarget, rate: f32) -> Self {
        Self {
            target,
            rate: rate.max(0.0),
            delay_after_damage: 0.0,
            delay_timer: 0.0,
            pending: 0.0,
            enabled: true,
        }
    }

    pub fn health(rate: f32) -> Self {
        Self::new(RegenTarget::Health, rate)
    }

    pub fn mana(rate: f32) -> Self {
        Self::new(RegenTarget::Mana, rate)
    }

    pub fn stamina(rate: f32) -> Self {
        Self::new(RegenTarget::Stamina, rate)
    }

    pub fn with_delay(mut self, delay: f32) -> Self {
        self.delay_after_damage = delay.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Call when the entity takes damage to pause regeneration.
    pub fn notify_damage(&mut self) {
        if self.delay_after_damage > 0.0 {
            self.delay_timer = self.delay_after_damage;
        }
    }

    /// Advance the regen timer. Accumulates `pending` when not suppressed.
    /// Call once per frame; drain `pending` afterward.
    pub fn tick(&mut self, dt: f32) {
        if !self.enabled {
            self.pending = 0.0;
            return;
        }

        if self.delay_timer > 0.0 {
            self.delay_timer = (self.delay_timer - dt).max(0.0);
            self.pending = 0.0;
            return;
        }

        self.pending = self.rate * dt;
    }

    /// Consume and return the accumulated regen for this frame.
    pub fn drain(&mut self) -> f32 {
        let amount = self.pending;
        self.pending = 0.0;
        amount
    }

    /// True when damage-delay countdown is active (regen suppressed).
    pub fn is_suppressed(&self) -> bool {
        self.delay_timer > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulates_per_tick() {
        let mut r = Regen::health(10.0);
        r.tick(1.0);
        assert!((r.pending - 10.0).abs() < 1e-5);
    }

    #[test]
    fn drain_clears_pending() {
        let mut r = Regen::health(10.0);
        r.tick(1.0);
        let amount = r.drain();
        assert!((amount - 10.0).abs() < 1e-5);
        assert_eq!(r.pending, 0.0);
    }

    #[test]
    fn delay_suppresses_regen() {
        let mut r = Regen::health(10.0).with_delay(2.0);
        r.notify_damage();
        r.tick(1.0); // delay: 2.0 → 1.0; suppressed
        assert_eq!(r.pending, 0.0);
    }

    #[test]
    fn delay_expires_and_regen_resumes() {
        let mut r = Regen::health(10.0).with_delay(1.0);
        r.notify_damage();
        r.tick(1.0); // delay expires
        r.tick(0.5);
        assert!((r.pending - 5.0).abs() < 1e-5);
    }

    #[test]
    fn no_delay_regen_always_active() {
        let mut r = Regen::mana(20.0);
        r.notify_damage(); // no-op since delay_after_damage == 0
        r.tick(0.5);
        assert!((r.pending - 10.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_produces_zero() {
        let mut r = Regen::stamina(10.0).disabled();
        r.tick(1.0);
        assert_eq!(r.pending, 0.0);
    }
}
