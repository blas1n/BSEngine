use bevy_ecs::prelude::Component;

/// Wind-burst emitter that applies a radial pushback impulse.
///
/// When the caller triggers a gust (`trigger()` returns `true`), the ability
/// system should push all entities within `radius` away from this entity with
/// force `force`. Subsequent triggers are gated by `cooldown` seconds.
///
/// `just_triggered` is set for one frame so VFX and audio hooks can fire.
/// `tick(dt)` advances the cooldown and clears single-frame flags.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Gust {
    /// Impulse magnitude applied to entities within `radius`.
    pub force: f32,
    /// Radius around the entity affected by the burst.
    pub radius: f32,
    /// Time between bursts in seconds. 0.0 = no cooldown.
    pub cooldown: f32,
    pub cooldown_timer: f32,
    pub just_triggered: bool,
    pub enabled: bool,
}

impl Gust {
    pub fn new(force: f32, radius: f32) -> Self {
        Self {
            force: force.max(0.0),
            radius: radius.max(0.0),
            cooldown: 0.0,
            cooldown_timer: 0.0,
            just_triggered: false,
            enabled: true,
        }
    }

    pub fn with_cooldown(mut self, cooldown: f32) -> Self {
        self.cooldown = cooldown.max(0.0);
        self
    }

    /// Attempt to fire a gust burst. Returns `true` when the burst fires
    /// and sets `just_triggered`; returns `false` when on cooldown or disabled.
    pub fn trigger(&mut self) -> bool {
        if !self.enabled || !self.is_ready() {
            return false;
        }

        self.just_triggered = true;
        self.cooldown_timer = self.cooldown;
        true
    }

    /// Advance the cooldown timer and clear single-frame flags.
    pub fn tick(&mut self, dt: f32) {
        self.just_triggered = false;

        if self.cooldown_timer > 0.0 {
            self.cooldown_timer -= dt;
            if self.cooldown_timer < 0.0 {
                self.cooldown_timer = 0.0;
            }
        }
    }

    /// Returns `true` when the gust is ready to fire (cooldown elapsed).
    pub fn is_ready(&self) -> bool {
        self.cooldown_timer <= 0.0
    }

    /// Fraction of cooldown elapsed [0.0 = just fired, 1.0 = ready again].
    pub fn cooldown_fraction(&self) -> f32 {
        if self.cooldown <= 0.0 {
            return 1.0;
        }
        1.0 - (self.cooldown_timer / self.cooldown).clamp(0.0, 1.0)
    }
}

impl Default for Gust {
    fn default() -> Self {
        Self::new(500.0, 5.0).with_cooldown(3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_fires_when_ready() {
        let mut g = Gust::new(500.0, 5.0);
        let fired = g.trigger();
        assert!(fired);
        assert!(g.just_triggered);
    }

    #[test]
    fn trigger_blocked_on_cooldown() {
        let mut g = Gust::new(500.0, 5.0).with_cooldown(2.0);
        g.trigger();
        let second = g.trigger();
        assert!(!second);
    }

    #[test]
    fn tick_reduces_cooldown() {
        let mut g = Gust::new(500.0, 5.0).with_cooldown(2.0);
        g.trigger();
        g.tick(1.0);
        assert!((g.cooldown_timer - 1.0).abs() < 1e-5);
        assert!(!g.is_ready());
    }

    #[test]
    fn tick_expires_cooldown() {
        let mut g = Gust::new(500.0, 5.0).with_cooldown(1.0);
        g.trigger();
        g.tick(1.5);
        assert!(g.is_ready());
        assert!((g.cooldown_timer).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_just_triggered() {
        let mut g = Gust::new(500.0, 5.0);
        g.trigger();
        assert!(g.just_triggered);
        g.tick(0.016);
        assert!(!g.just_triggered);
    }

    #[test]
    fn trigger_fires_again_after_cooldown() {
        let mut g = Gust::new(500.0, 5.0).with_cooldown(1.0);
        g.trigger();
        g.tick(1.5);
        let second = g.trigger();
        assert!(second);
    }

    #[test]
    fn cooldown_fraction_at_half() {
        let mut g = Gust::new(500.0, 5.0).with_cooldown(2.0);
        g.trigger();
        g.tick(1.0); // 1s elapsed of 2s cooldown
        let frac = g.cooldown_fraction();
        assert!((frac - 0.5).abs() < 1e-3);
    }

    #[test]
    fn no_cooldown_always_ready() {
        let mut g = Gust::new(500.0, 5.0);
        g.trigger();
        assert!(g.is_ready()); // no cooldown set → immediately ready again
        assert!((g.cooldown_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_trigger_no_op() {
        let mut g = Gust::new(500.0, 5.0);
        g.enabled = false;
        let fired = g.trigger();
        assert!(!fired);
        assert!(!g.just_triggered);
    }
}
