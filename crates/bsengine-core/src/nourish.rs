use bevy_ecs::prelude::Component;

/// Hunger / satiety loop: `satiety` [0.0, 1.0] decays over time at
/// `decay_rate` units per second. The entity must call `feed(amount)` to
/// replenish satiety before it runs out. While nourished, regeneration rates
/// are amplified by a factor that scales linearly with satiety.
///
/// `feed(amount)` adds to `satiety` (capped at 1.0). No-op when disabled or
/// `amount ≤ 0`.
///
/// `tick(dt)` clears `just_starved` at the start, then decays satiety by
/// `decay_rate * dt` (floored at 0.0). Fires `just_starved` on the first
/// transition to 0.
///
/// `is_starving()` returns `satiety ≤ 0.0 && enabled`.
///
/// `regen_bonus()` returns `satiety * regen_scale` when enabled; 0.0 when
/// disabled. This is an additive factor layered on top of base regen — at
/// full satiety it equals `regen_scale`.
///
/// `effective_regen(base)` returns `base * (1.0 + regen_bonus())` when
/// enabled and `regen_bonus() > 0`; returns `base` otherwise.
///
/// Distinct from `Regen` (unconditional passive HP recovery unrelated to
/// feeding), `Heal` (instant HP restoration), and `Repose` (voluntary rest
/// state that amplifies regen in exchange for inactivity): Nourish is a
/// **sustained hunger loop** — the player must actively feed the entity to
/// maintain the satiety bonus; it is always decaying unless replenished.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Nourish {
    /// Current satiety [0.0 = starving, 1.0 = full].
    pub satiety: f32,
    /// Satiety lost per second. Clamped ≥ 0.0.
    pub decay_rate: f32,
    /// Maximum additive regen bonus at full satiety. Clamped ≥ 0.0.
    /// e.g. `0.5` → +50 % regen at satiety 1.0; +25 % at satiety 0.5.
    pub regen_scale: f32,
    pub just_starved: bool,
    pub enabled: bool,
}

impl Nourish {
    pub fn new(decay_rate: f32, regen_scale: f32) -> Self {
        Self {
            satiety: 1.0,
            decay_rate: decay_rate.max(0.0),
            regen_scale: regen_scale.max(0.0),
            just_starved: false,
            enabled: true,
        }
    }

    /// Replenish satiety by `amount` (capped at 1.0). No-op when disabled
    /// or `amount ≤ 0`.
    pub fn feed(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        self.satiety = (self.satiety + amount).min(1.0);
    }

    /// Clear one-frame flags, then decay satiety by `decay_rate * dt`.
    /// Fires `just_starved` on the transition to 0.
    pub fn tick(&mut self, dt: f32) {
        self.just_starved = false;

        if self.satiety > 0.0 && self.decay_rate > 0.0 {
            let was_nourished = !self.is_starving();
            self.satiety = (self.satiety - self.decay_rate * dt).max(0.0);
            if was_nourished && self.is_starving() {
                self.just_starved = true;
            }
        }
    }

    /// `true` when satiety has reached 0 and the component is enabled.
    pub fn is_starving(&self) -> bool {
        self.satiety <= 0.0 && self.enabled
    }

    /// Additive regen bonus from satiety: `satiety * regen_scale` when
    /// enabled; 0.0 when disabled.
    pub fn regen_bonus(&self) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        self.satiety * self.regen_scale
    }

    /// Base regen scaled by current satiety: `base * (1.0 + regen_bonus())`
    /// when enabled and bonus > 0; returns `base` otherwise.
    pub fn effective_regen(&self, base: f32) -> f32 {
        let bonus = self.regen_bonus();
        if bonus > 0.0 {
            base * (1.0 + bonus)
        } else {
            base
        }
    }

    /// Satiety fraction [0.0 = empty, 1.0 = full]. Always returns satiety
    /// directly since it is already clamped.
    pub fn satiety_fraction(&self) -> f32 {
        self.satiety
    }
}

impl Default for Nourish {
    fn default() -> Self {
        Self::new(0.1, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_full() {
        let n = Nourish::new(0.1, 0.5);
        assert!((n.satiety - 1.0).abs() < 1e-5);
        assert!(!n.is_starving());
    }

    #[test]
    fn feed_increases_satiety() {
        let mut n = Nourish::new(0.1, 0.5);
        n.satiety = 0.3;
        n.feed(0.4);
        assert!((n.satiety - 0.7).abs() < 1e-4);
    }

    #[test]
    fn feed_caps_at_one() {
        let mut n = Nourish::new(0.1, 0.5);
        n.satiety = 0.8;
        n.feed(0.5); // would be 1.3
        assert!((n.satiety - 1.0).abs() < 1e-5);
    }

    #[test]
    fn feed_no_op_when_disabled() {
        let mut n = Nourish::new(0.1, 0.5);
        n.satiety = 0.0;
        n.enabled = false;
        n.feed(0.5);
        assert_eq!(n.satiety, 0.0);
    }

    #[test]
    fn feed_no_op_at_zero_amount() {
        let mut n = Nourish::new(0.1, 0.5);
        n.satiety = 0.5;
        n.feed(0.0);
        assert!((n.satiety - 0.5).abs() < 1e-5);
    }

    #[test]
    fn feed_no_op_at_negative_amount() {
        let mut n = Nourish::new(0.1, 0.5);
        n.satiety = 0.5;
        n.feed(-0.2);
        assert!((n.satiety - 0.5).abs() < 1e-5);
    }

    #[test]
    fn tick_decays_satiety() {
        let mut n = Nourish::new(0.2, 0.5);
        n.tick(1.0); // 1.0 - 0.2 = 0.8
        assert!((n.satiety - 0.8).abs() < 1e-4);
    }

    #[test]
    fn tick_floors_at_zero() {
        let mut n = Nourish::new(2.0, 0.5);
        n.tick(1.0); // would go negative
        assert_eq!(n.satiety, 0.0);
    }

    #[test]
    fn tick_fires_just_starved_on_transition() {
        let mut n = Nourish::new(1.0, 0.5);
        n.satiety = 0.5;
        n.tick(1.0); // hits 0
        assert!(n.just_starved);
        assert!(n.is_starving());
    }

    #[test]
    fn tick_no_just_starved_when_already_starving() {
        let mut n = Nourish::new(1.0, 0.5);
        n.satiety = 0.0;
        n.tick(1.0);
        assert!(!n.just_starved);
    }

    #[test]
    fn tick_clears_just_starved_next_frame() {
        let mut n = Nourish::new(1.0, 0.5);
        n.satiety = 0.5;
        n.tick(1.0); // starves
        n.tick(0.016);
        assert!(!n.just_starved);
    }

    #[test]
    fn tick_no_decay_when_rate_zero() {
        let mut n = Nourish::new(0.0, 0.5);
        n.tick(100.0);
        assert!((n.satiety - 1.0).abs() < 1e-5);
    }

    #[test]
    fn is_starving_false_when_disabled() {
        let mut n = Nourish::new(0.1, 0.5);
        n.satiety = 0.0;
        n.enabled = false;
        assert!(!n.is_starving());
    }

    #[test]
    fn regen_bonus_at_full_satiety() {
        let n = Nourish::new(0.1, 0.5);
        // 1.0 * 0.5 = 0.5
        assert!((n.regen_bonus() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn regen_bonus_at_half_satiety() {
        let mut n = Nourish::new(0.1, 0.5);
        n.satiety = 0.5;
        // 0.5 * 0.5 = 0.25
        assert!((n.regen_bonus() - 0.25).abs() < 1e-5);
    }

    #[test]
    fn regen_bonus_zero_when_starving() {
        let mut n = Nourish::new(0.1, 0.5);
        n.satiety = 0.0;
        assert!(n.regen_bonus().abs() < 1e-5);
    }

    #[test]
    fn regen_bonus_zero_when_disabled() {
        let mut n = Nourish::new(0.1, 0.5);
        n.enabled = false;
        assert!(n.regen_bonus().abs() < 1e-5);
    }

    #[test]
    fn effective_regen_at_full_satiety() {
        let n = Nourish::new(0.1, 0.5);
        // 10 * (1 + 0.5) = 15
        assert!((n.effective_regen(10.0) - 15.0).abs() < 1e-3);
    }

    #[test]
    fn effective_regen_at_half_satiety() {
        let mut n = Nourish::new(0.1, 0.5);
        n.satiety = 0.5;
        // 10 * (1 + 0.25) = 12.5
        assert!((n.effective_regen(10.0) - 12.5).abs() < 1e-3);
    }

    #[test]
    fn effective_regen_base_when_starving() {
        let mut n = Nourish::new(0.1, 0.5);
        n.satiety = 0.0;
        assert!((n.effective_regen(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn effective_regen_base_when_disabled() {
        let n = Nourish::new(0.1, 0.5);
        let mut n2 = n.clone();
        n2.enabled = false;
        assert!((n2.effective_regen(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn satiety_fraction_matches_satiety() {
        let mut n = Nourish::new(0.1, 0.5);
        n.satiety = 0.6;
        assert!((n.satiety_fraction() - 0.6).abs() < 1e-5);
    }

    #[test]
    fn decay_rate_clamped_non_negative() {
        let n = Nourish::new(-1.0, 0.5);
        assert_eq!(n.decay_rate, 0.0);
    }

    #[test]
    fn regen_scale_clamped_non_negative() {
        let n = Nourish::new(0.1, -0.5);
        assert_eq!(n.regen_scale, 0.0);
    }

    #[test]
    fn feed_after_starvation_exits_starving() {
        let mut n = Nourish::new(1.0, 0.5);
        n.tick(2.0); // drains to 0
        n.tick(0.016);
        n.feed(0.5); // refill
        assert!(!n.is_starving());
        assert!((n.satiety - 0.5).abs() < 1e-4);
    }
}
