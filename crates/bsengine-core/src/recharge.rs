use bevy_ecs::prelude::Component;

/// Single-resource charge pool that regenerates over time.
///
/// Unlike `Cooldown` (multiple abilities keyed by id), `Recharge` models one
/// charge resource that refills at a fixed rate — useful for energy weapons,
/// power shields, and single-slot abilities.
///
/// `consume(amount)` subtracts from `current` and returns `true` only when
/// sufficient charge is available. `tick(dt)` advances the regen and sets
/// `just_recharged` when the pool first reaches `max`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Recharge {
    pub current: f32,
    pub max: f32,
    /// Charge units restored per second.
    pub rate: f32,
    pub just_recharged: bool,
    pub just_depleted: bool,
    pub enabled: bool,
}

impl Recharge {
    pub fn new(max: f32, rate: f32) -> Self {
        Self {
            current: max.max(0.0),
            max: max.max(0.0),
            rate: rate.max(0.0),
            just_recharged: false,
            just_depleted: false,
            enabled: true,
        }
    }

    /// Start fully depleted (e.g. just picked up and must charge from zero).
    pub fn empty(max: f32, rate: f32) -> Self {
        let mut s = Self::new(max, rate);
        s.current = 0.0;
        s
    }

    /// Consume `amount` of charge. Returns `true` and subtracts if available;
    /// returns `false` without modifying `current` when insufficient.
    pub fn consume(&mut self, amount: f32) -> bool {
        if !self.enabled || amount <= 0.0 {
            return true; // zero-cost consume always succeeds
        }

        if self.current >= amount {
            let was_full = self.is_full();
            self.current -= amount;
            if was_full {
                self.just_depleted = true;
            }
            return true;
        }

        false
    }

    /// Manually add `amount` of charge (e.g., from a pickup or ability).
    pub fn add(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }

        let was_full = self.is_full();
        self.current = (self.current + amount).min(self.max);
        if !was_full && self.is_full() {
            self.just_recharged = true;
        }
    }

    /// Advance natural regen by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        self.just_recharged = false;
        self.just_depleted = false;

        if self.rate > 0.0 && self.current < self.max {
            let was_full = self.is_full();
            self.current = (self.current + self.rate * dt).min(self.max);
            if !was_full && self.is_full() {
                self.just_recharged = true;
            }
        }
    }

    pub fn is_full(&self) -> bool {
        self.current >= self.max
    }

    pub fn is_empty(&self) -> bool {
        self.current <= 0.0
    }

    /// Current charge as a fraction of max [0.0, 1.0].
    pub fn fraction(&self) -> f32 {
        if self.max <= 0.0 {
            return 0.0;
        }
        (self.current / self.max).clamp(0.0, 1.0)
    }
}

impl Default for Recharge {
    fn default() -> Self {
        Self::new(100.0, 10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_full() {
        let r = Recharge::new(100.0, 5.0);
        assert!(r.is_full());
        assert!((r.fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn empty_starts_depleted() {
        let r = Recharge::empty(100.0, 5.0);
        assert!(r.is_empty());
        assert!((r.fraction()).abs() < 1e-5);
    }

    #[test]
    fn consume_succeeds_when_sufficient() {
        let mut r = Recharge::new(100.0, 5.0);
        let ok = r.consume(30.0);
        assert!(ok);
        assert!((r.current - 70.0).abs() < 1e-5);
    }

    #[test]
    fn consume_fails_when_insufficient() {
        let mut r = Recharge::empty(100.0, 5.0);
        let ok = r.consume(10.0);
        assert!(!ok);
        assert!((r.current).abs() < 1e-5);
    }

    #[test]
    fn consume_from_full_sets_just_depleted() {
        let mut r = Recharge::new(100.0, 5.0);
        r.consume(1.0);
        assert!(r.just_depleted);
    }

    #[test]
    fn add_fills_pool() {
        let mut r = Recharge::empty(100.0, 5.0);
        r.add(40.0);
        assert!((r.current - 40.0).abs() < 1e-5);
    }

    #[test]
    fn add_caps_at_max_and_sets_just_recharged() {
        let mut r = Recharge::empty(100.0, 5.0);
        r.add(150.0);
        assert!((r.current - 100.0).abs() < 1e-5);
        assert!(r.just_recharged);
    }

    #[test]
    fn tick_recharges_pool() {
        let mut r = Recharge::empty(100.0, 10.0);
        r.tick(2.0);
        assert!((r.current - 20.0).abs() < 1e-5);
    }

    #[test]
    fn tick_sets_just_recharged_at_full() {
        let mut r = Recharge::empty(100.0, 50.0);
        r.tick(2.0);
        assert!(r.is_full());
        assert!(r.just_recharged);
    }

    #[test]
    fn tick_clears_flags_each_frame() {
        let mut r = Recharge::empty(100.0, 50.0);
        r.tick(2.0);
        assert!(r.just_recharged);
        r.tick(0.016);
        assert!(!r.just_recharged);
    }

    #[test]
    fn fraction_at_half() {
        let mut r = Recharge::empty(100.0, 5.0);
        r.add(50.0);
        assert!((r.fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn disabled_consume_no_op() {
        let mut r = Recharge::new(100.0, 5.0);
        r.enabled = false;
        let ok = r.consume(30.0);
        assert!(ok); // returns true (free) but doesn't deduct
        assert!((r.current - 100.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_add_no_op() {
        let mut r = Recharge::empty(100.0, 5.0);
        r.enabled = false;
        r.add(50.0);
        assert!((r.current).abs() < 1e-5);
    }
}
