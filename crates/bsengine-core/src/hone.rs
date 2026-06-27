use bevy_ecs::prelude::Component;

/// Weapon-sharpening accumulator: entity's effective damage scales with
/// `sharpness` [0.0, 1.0], which rises on each `strike()` call and decays
/// passively when idle.
///
/// `strike()` adds `gain_per_hit` to `sharpness` (clamped at 1.0) and sets
/// `just_peaked` on the 0-to-positive transition that first reaches 1.0.
/// `tick(dt)` decays `sharpness` by `decay_rate * dt` and clears one-frame
/// flags at the start of each call.
///
/// `effective_damage(base)` returns `base * (1 + damage_bonus * sharpness)`
/// when enabled, giving full bonus only at peak sharpness.
///
/// Distinct from `Reckless` (constant trade-off), `Fervor` (stack-on-kill),
/// and `Amplify` (flat multiplier): Hone is a **use-or-lose sharpness meter**
/// — it rewards consistent pressure but dull if the entity stops attacking.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Hone {
    /// Current sharpness [0.0, 1.0].
    pub sharpness: f32,
    /// Sharpness gained per `strike()` call. Clamped ≥ 0.0.
    pub gain_per_hit: f32,
    /// Sharpness lost per second while not striking. Clamped ≥ 0.0.
    pub decay_rate: f32,
    /// Maximum damage bonus fraction at full sharpness. Clamped ≥ 0.0.
    pub damage_bonus: f32,
    pub just_peaked: bool,
    pub enabled: bool,
}

impl Hone {
    pub fn new(gain_per_hit: f32, decay_rate: f32, damage_bonus: f32) -> Self {
        Self {
            sharpness: 0.0,
            gain_per_hit: gain_per_hit.max(0.0),
            decay_rate: decay_rate.max(0.0),
            damage_bonus: damage_bonus.max(0.0),
            just_peaked: false,
            enabled: true,
        }
    }

    /// Register a successful hit: increases `sharpness` by `gain_per_hit`
    /// (clamped at 1.0). Sets `just_peaked` on the transition to 1.0.
    /// No-op when disabled.
    pub fn strike(&mut self) {
        if !self.enabled {
            return;
        }
        let was_below_peak = self.sharpness < 1.0;
        self.sharpness = (self.sharpness + self.gain_per_hit).min(1.0);
        if was_below_peak && self.sharpness >= 1.0 {
            self.just_peaked = true;
        }
    }

    /// Decay sharpness by `decay_rate * dt`. Clears one-frame flags at the
    /// start of each tick. Sharpness is floored at 0.0.
    pub fn tick(&mut self, dt: f32) {
        self.just_peaked = false;

        if self.sharpness > 0.0 {
            self.sharpness -= self.decay_rate * dt;
            if self.sharpness < 0.0 {
                self.sharpness = 0.0;
            }
        }
    }

    pub fn is_sharp(&self) -> bool {
        self.sharpness > 0.0
    }

    /// Effective outgoing damage scaled by current sharpness.
    /// Returns `base * (1 + damage_bonus * sharpness)` when enabled;
    /// returns `base` when disabled or sharpness is 0.
    pub fn effective_damage(&self, base: f32) -> f32 {
        if self.enabled && self.is_sharp() {
            base * (1.0 + self.damage_bonus * self.sharpness)
        } else {
            base
        }
    }
}

impl Default for Hone {
    fn default() -> Self {
        Self::new(0.15, 0.1, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strike_increases_sharpness() {
        let mut h = Hone::new(0.2, 0.0, 0.5);
        h.strike();
        assert!((h.sharpness - 0.2).abs() < 1e-5);
        assert!(h.is_sharp());
    }

    #[test]
    fn strike_clamps_at_one() {
        let mut h = Hone::new(0.6, 0.0, 0.5);
        h.strike();
        h.strike();
        assert!((h.sharpness - 1.0).abs() < 1e-5);
    }

    #[test]
    fn strike_sets_just_peaked_on_reaching_one() {
        let mut h = Hone::new(0.6, 0.0, 0.5);
        h.strike();
        assert!(!h.just_peaked);
        h.strike();
        assert!(h.just_peaked);
    }

    #[test]
    fn strike_no_just_peaked_when_already_at_one() {
        let mut h = Hone::new(0.6, 0.0, 0.5);
        h.strike();
        h.strike(); // peaks here
        h.tick(0.016);
        h.strike(); // still at 1.0 → no peak event
        assert!(!h.just_peaked);
    }

    #[test]
    fn tick_decays_sharpness() {
        let mut h = Hone::new(0.5, 0.2, 0.5);
        h.strike();
        h.tick(1.0); // 0.5 - 0.2 = 0.3
        assert!((h.sharpness - 0.3).abs() < 1e-5);
    }

    #[test]
    fn tick_floors_at_zero() {
        let mut h = Hone::new(0.3, 1.0, 0.5);
        h.strike();
        h.tick(5.0);
        assert_eq!(h.sharpness, 0.0);
        assert!(!h.is_sharp());
    }

    #[test]
    fn tick_clears_just_peaked() {
        let mut h = Hone::new(1.0, 0.0, 0.5);
        h.strike();
        h.tick(0.016);
        assert!(!h.just_peaked);
    }

    #[test]
    fn tick_no_op_when_zero() {
        let mut h = Hone::new(0.2, 0.5, 0.5);
        h.tick(10.0);
        assert_eq!(h.sharpness, 0.0);
    }

    #[test]
    fn effective_damage_scales_with_sharpness() {
        let mut h = Hone::new(1.0, 0.0, 0.5);
        h.strike(); // sharpness = 1.0
                    // 100 * (1 + 0.5 * 1.0) = 150
        assert!((h.effective_damage(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_damage_at_half_sharpness() {
        let mut h = Hone::new(0.5, 0.0, 0.5);
        h.strike(); // sharpness = 0.5
                    // 100 * (1 + 0.5 * 0.5) = 125
        assert!((h.effective_damage(100.0) - 125.0).abs() < 1e-3);
    }

    #[test]
    fn effective_damage_base_when_not_sharp() {
        let h = Hone::new(0.2, 0.0, 0.5);
        assert!((h.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_strike_no_op() {
        let mut h = Hone::new(0.3, 0.0, 0.5);
        h.enabled = false;
        h.strike();
        assert_eq!(h.sharpness, 0.0);
    }

    #[test]
    fn disabled_effective_damage_base() {
        let mut h = Hone::new(1.0, 0.0, 0.5);
        h.strike();
        h.enabled = false;
        assert!((h.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn multiple_strikes_accumulate() {
        let mut h = Hone::new(0.1, 0.0, 1.0);
        for _ in 0..5 {
            h.strike();
        }
        assert!((h.sharpness - 0.5).abs() < 1e-4);
        // 100 * (1 + 1.0 * 0.5) = 150
        assert!((h.effective_damage(100.0) - 150.0).abs() < 1e-3);
    }
}
