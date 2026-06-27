use bevy_ecs::prelude::Component;

/// Energy-drain debuff that suppresses mana/ability regeneration and shrinks
/// the entity's effective energy pool for its duration.
///
/// While enervated, the resource system should:
/// - Multiply the entity's energy regeneration rate by `effective_regen(base)`
/// - Cap the entity's effective max pool at `effective_max_pool(base_max)`
///
/// Both return the base value unchanged when the enervate is inactive.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_restored` when the effect wears off.
///
/// Distinct from `Exhaustion` (stamina/physical fatigue), `Silence` (blocks
/// ability use entirely), and `Suppress` (potency reduction on abilities):
/// Enervate is specifically a mana/energy resource debuff — abilities can still
/// fire, but the entity runs dry faster and recovers slower.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Enervate {
    pub duration: f32,
    pub timer: f32,
    /// Multiplier on energy regeneration rate. Clamped to [0.0, 1.0].
    /// e.g. 0.0 = no regeneration at all while enervated.
    pub regen_fraction: f32,
    /// Multiplier on the maximum energy pool. Clamped to [0.0, 1.0].
    /// e.g. 0.5 = effective max pool is halved.
    pub max_pool_fraction: f32,
    pub just_enervated: bool,
    pub just_restored: bool,
    pub enabled: bool,
}

impl Enervate {
    pub fn new(regen_fraction: f32, max_pool_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            regen_fraction: regen_fraction.clamp(0.0, 1.0),
            max_pool_fraction: max_pool_fraction.clamp(0.0, 1.0),
            just_enervated: false,
            just_restored: false,
            enabled: true,
        }
    }

    /// Apply or extend the enervate for `duration` seconds. High-watermark:
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
                self.just_enervated = true;
            }
        }
    }

    /// Remove the enervate immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_restored = true;
        }
    }

    /// Advance the timer; sets `just_restored` when the effect expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_enervated = false;
        self.just_restored = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_restored = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective energy regeneration rate. Returns `base * regen_fraction`
    /// while active, `base` otherwise.
    pub fn effective_regen(&self, base: f32) -> f32 {
        if self.is_active() {
            base * self.regen_fraction
        } else {
            base
        }
    }

    /// Effective maximum energy pool. Returns `base_max * max_pool_fraction`
    /// while active, `base_max` otherwise.
    pub fn effective_max_pool(&self, base_max: f32) -> f32 {
        if self.is_active() {
            base_max * self.max_pool_fraction
        } else {
            base_max
        }
    }

    /// Fraction of the enervate duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Enervate {
    fn default() -> Self {
        Self::new(0.0, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_enervate() {
        let mut e = Enervate::new(0.0, 0.5);
        e.apply(3.0);
        assert!(e.is_active());
        assert!(e.just_enervated);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut e = Enervate::new(0.0, 0.5);
        e.apply(2.0);
        e.tick(0.016);
        e.apply(5.0);
        assert!((e.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut e = Enervate::new(0.0, 0.5);
        e.apply(5.0);
        e.apply(2.0);
        assert!((e.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_enervate() {
        let mut e = Enervate::new(0.0, 0.5);
        e.apply(1.0);
        e.tick(1.1);
        assert!(!e.is_active());
        assert!(e.just_restored);
    }

    #[test]
    fn clear_ends_early() {
        let mut e = Enervate::new(0.0, 0.5);
        e.apply(5.0);
        e.clear();
        assert!(!e.is_active());
        assert!(e.just_restored);
    }

    #[test]
    fn effective_regen_while_active_zero() {
        let mut e = Enervate::new(0.0, 0.5);
        e.apply(3.0);
        assert!((e.effective_regen(10.0) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn effective_regen_while_active_partial() {
        let mut e = Enervate::new(0.3, 0.5);
        e.apply(3.0);
        assert!((e.effective_regen(10.0) - 3.0).abs() < 1e-4);
    }

    #[test]
    fn effective_regen_when_inactive() {
        let e = Enervate::new(0.0, 0.5);
        assert!((e.effective_regen(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn effective_max_pool_while_active() {
        let mut e = Enervate::new(0.0, 0.5);
        e.apply(3.0);
        assert!((e.effective_max_pool(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_max_pool_when_inactive() {
        let e = Enervate::new(0.0, 0.5);
        assert!((e.effective_max_pool(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut e = Enervate::new(0.0, 0.5);
        e.apply(2.0);
        e.tick(1.0);
        assert!((e.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut e = Enervate::new(0.0, 0.5);
        e.enabled = false;
        e.apply(5.0);
        assert!(!e.is_active());
    }

    #[test]
    fn tick_clears_just_enervated() {
        let mut e = Enervate::new(0.0, 0.5);
        e.apply(3.0);
        e.tick(0.016);
        assert!(!e.just_enervated);
    }

    #[test]
    fn fractions_clamped() {
        let e = Enervate::new(-0.5, 1.5);
        assert!((e.regen_fraction - 0.0).abs() < 1e-5);
        assert!((e.max_pool_fraction - 1.0).abs() < 1e-5);
    }
}
