use bevy_ecs::prelude::Component;

/// Persistent wet-state modifier: tracks how saturated an entity is with
/// water (or similar liquid). While soaked, incoming fire damage is reduced
/// and incoming lightning damage is amplified, both proportional to
/// `soak_level`.
///
/// `absorb(amount)` increases `soak_level` (clamped at 1.0) and sets
/// `just_soaked` the frame saturation transitions from 0 to positive.
/// `tick(dt)` decays `soak_level` by `decay_rate * dt` and sets `just_dried`
/// when it reaches 0 from above. One-frame flags are cleared at the start of
/// each `tick`.
///
/// Distinct from `Drench` (the projectile/area source that applies soak),
/// `Douse` (extinguishes a burn effect), and `Freeze` (solidifies when
/// temperature drops): Soak is the **persistent wet-state modifier** on the
/// recipient — it accumulates, decays naturally, and gates elemental
/// resistances and vulnerabilities.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Soak {
    /// Current saturation level [0.0, 1.0].
    pub soak_level: f32,
    /// Rate of saturation loss per second. Clamped ≥ 0.0.
    pub decay_rate: f32,
    /// Fraction of fire damage blocked per unit of soak_level.
    /// e.g. 0.5 means fully soaked blocks 50% of fire damage.
    pub fire_resistance: f32,
    /// Fraction of extra lightning damage taken per unit of soak_level.
    /// e.g. 0.5 means fully soaked takes 50% more lightning damage.
    pub lightning_amplify: f32,
    pub just_soaked: bool,
    pub just_dried: bool,
    pub enabled: bool,
}

impl Soak {
    pub fn new(decay_rate: f32, fire_resistance: f32, lightning_amplify: f32) -> Self {
        Self {
            soak_level: 0.0,
            decay_rate: decay_rate.max(0.0),
            fire_resistance: fire_resistance.clamp(0.0, 1.0),
            lightning_amplify: lightning_amplify.max(0.0),
            just_soaked: false,
            just_dried: false,
            enabled: true,
        }
    }

    /// Increase saturation by `amount` (clamped to [0.0, 1.0]). Sets
    /// `just_soaked` on the 0 → positive transition. No-op when disabled.
    pub fn absorb(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_dry = self.soak_level <= 0.0;
        self.soak_level = (self.soak_level + amount).min(1.0);
        if was_dry && self.soak_level > 0.0 {
            self.just_soaked = true;
        }
    }

    /// Advance the saturation decay. Sets `just_dried` when the level reaches
    /// 0. Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_soaked = false;
        self.just_dried = false;

        if self.soak_level > 0.0 {
            self.soak_level -= self.decay_rate * dt;
            if self.soak_level <= 0.0 {
                self.soak_level = 0.0;
                self.just_dried = true;
            }
        }
    }

    pub fn is_soaked(&self) -> bool {
        self.soak_level > 0.0
    }

    /// Effective incoming fire damage after soak reduction.
    /// Returns `base * (1 - fire_resistance * soak_level)` when soaked and
    /// enabled, floored at `0.0`. Returns `base` otherwise.
    pub fn effective_fire_damage(&self, base: f32) -> f32 {
        if self.is_soaked() && self.enabled {
            (base * (1.0 - self.fire_resistance * self.soak_level)).max(0.0)
        } else {
            base
        }
    }

    /// Effective incoming lightning damage with soak amplification.
    /// Returns `base * (1 + lightning_amplify * soak_level)` when soaked and
    /// enabled. Returns `base` otherwise.
    pub fn effective_lightning_damage(&self, base: f32) -> f32 {
        if self.is_soaked() && self.enabled {
            base * (1.0 + self.lightning_amplify * self.soak_level)
        } else {
            base
        }
    }
}

impl Default for Soak {
    fn default() -> Self {
        Self::new(0.1, 0.5, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absorb_increases_soak_level() {
        let mut s = Soak::new(0.1, 0.5, 0.5);
        s.absorb(0.4);
        assert!((s.soak_level - 0.4).abs() < 1e-5);
        assert!(s.is_soaked());
        assert!(s.just_soaked);
    }

    #[test]
    fn absorb_clamps_at_one() {
        let mut s = Soak::new(0.1, 0.5, 0.5);
        s.absorb(0.8);
        s.absorb(0.5); // would overflow
        assert!((s.soak_level - 1.0).abs() < 1e-5);
    }

    #[test]
    fn absorb_no_just_soaked_when_already_wet() {
        let mut s = Soak::new(0.1, 0.5, 0.5);
        s.absorb(0.3);
        s.tick(0.016);
        s.absorb(0.2);
        assert!(!s.just_soaked);
    }

    #[test]
    fn tick_decays_soak_level() {
        let mut s = Soak::new(0.5, 0.5, 0.5);
        s.absorb(1.0);
        s.tick(1.0);
        assert!((s.soak_level - 0.5).abs() < 1e-5);
    }

    #[test]
    fn tick_sets_just_dried_on_zero() {
        let mut s = Soak::new(1.0, 0.5, 0.5);
        s.absorb(0.5);
        s.tick(1.0); // 0.5 - 1.0*1.0 = negative → clamped to 0
        assert_eq!(s.soak_level, 0.0);
        assert!(s.just_dried);
        assert!(!s.is_soaked());
    }

    #[test]
    fn tick_clears_just_soaked() {
        let mut s = Soak::new(0.1, 0.5, 0.5);
        s.absorb(0.5);
        s.tick(0.016);
        assert!(!s.just_soaked);
    }

    #[test]
    fn tick_clears_just_dried() {
        let mut s = Soak::new(1.0, 0.5, 0.5);
        s.absorb(0.5);
        s.tick(1.0);
        s.tick(0.016);
        assert!(!s.just_dried);
    }

    #[test]
    fn effective_fire_damage_reduced_when_soaked() {
        let mut s = Soak::new(0.1, 0.5, 0.5);
        s.absorb(1.0); // fully soaked
                       // base=100, 100 * (1 - 0.5*1.0) = 50
        assert!((s.effective_fire_damage(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_fire_damage_base_when_dry() {
        let s = Soak::new(0.1, 0.5, 0.5);
        assert!((s.effective_fire_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_fire_damage_floored_at_zero() {
        let mut s = Soak::new(0.1, 1.0, 0.5); // full resistance
        s.absorb(1.0);
        assert!((s.effective_fire_damage(100.0)).abs() < 1e-5);
    }

    #[test]
    fn effective_lightning_damage_amplified_when_soaked() {
        let mut s = Soak::new(0.1, 0.5, 0.5);
        s.absorb(1.0);
        // base=100, 100 * (1 + 0.5*1.0) = 150
        assert!((s.effective_lightning_damage(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_lightning_damage_base_when_dry() {
        let s = Soak::new(0.1, 0.5, 0.5);
        assert!((s.effective_lightning_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn partial_soak_scales_proportionally() {
        let mut s = Soak::new(0.1, 0.5, 0.5);
        s.absorb(0.5); // half soaked
                       // fire: 100 * (1 - 0.5*0.5) = 100 * 0.75 = 75
        assert!((s.effective_fire_damage(100.0) - 75.0).abs() < 1e-3);
        // lightning: 100 * (1 + 0.5*0.5) = 100 * 1.25 = 125
        assert!((s.effective_lightning_damage(100.0) - 125.0).abs() < 1e-3);
    }

    #[test]
    fn disabled_absorb_no_op() {
        let mut s = Soak::new(0.1, 0.5, 0.5);
        s.enabled = false;
        s.absorb(0.5);
        assert_eq!(s.soak_level, 0.0);
    }

    #[test]
    fn disabled_effective_fire_damage_base() {
        let mut s = Soak::new(0.1, 0.5, 0.5);
        s.absorb(1.0);
        s.enabled = false;
        assert!((s.effective_fire_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_effective_lightning_damage_base() {
        let mut s = Soak::new(0.1, 0.5, 0.5);
        s.absorb(1.0);
        s.enabled = false;
        assert!((s.effective_lightning_damage(100.0) - 100.0).abs() < 1e-5);
    }
}
