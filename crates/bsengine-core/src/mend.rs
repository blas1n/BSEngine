use bevy_ecs::prelude::Component;

/// Targeted heal-over-time injection: `mend(amount)` loads the `mend_pool`
/// with pending healing. Each `tick(dt)` call drains up to `rate * dt` from
/// the pool and returns the actual HP healed that frame. The pool is consumed
/// by healing, not by time — it persists until fully spent, unlike a
/// duration-based regen.
///
/// `just_depleted` fires on the frame the pool empties completely. Multiple
/// `mend()` calls in the same frame stack (the amounts are summed).
///
/// Healing via `tick(dt)` is a no-op when disabled or the pool is empty.
/// `mend()` is a no-op when disabled.
///
/// Distinct from `Regen` (unconditional HP regen each tick), `Absorption`
/// (pre-damage shielding), and `Revive` (post-death resurrection): Mend is
/// a **finite-pool heal-over-time injection** — an explicit heal amount is
/// committed and dispensed gradually at a controlled rate.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Mend {
    /// Pending healing remaining in the pool. Clamped ≥ 0.0.
    pub mend_pool: f32,
    /// Maximum healing dispensed per second. Clamped ≥ 0.0.
    pub rate: f32,
    pub just_depleted: bool,
    pub enabled: bool,
}

impl Mend {
    pub fn new(rate: f32) -> Self {
        Self {
            mend_pool: 0.0,
            rate: rate.max(0.0),
            just_depleted: false,
            enabled: true,
        }
    }

    /// Load `amount` HP into the mend pool. Amounts stack across multiple
    /// calls in the same frame. No-op when disabled or `amount ≤ 0`.
    pub fn mend(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        self.mend_pool += amount;
    }

    /// Dispense up to `rate * dt` HP from the pool. Returns the actual
    /// amount healed this frame (≥ 0.0). Sets `just_depleted` when the pool
    /// empties. Clears one-frame flags at the start of each tick.
    /// Returns 0.0 when disabled or the pool is empty.
    pub fn tick(&mut self, dt: f32) -> f32 {
        self.just_depleted = false;

        if !self.enabled || self.mend_pool <= 0.0 {
            return 0.0;
        }

        let heal = (self.rate * dt).min(self.mend_pool);
        self.mend_pool -= heal;
        if self.mend_pool <= 0.0 {
            self.mend_pool = 0.0;
            self.just_depleted = true;
        }
        heal
    }

    /// `true` while there is pending healing in the pool.
    pub fn is_mending(&self) -> bool {
        self.mend_pool > 0.0
    }
}

impl Default for Mend {
    fn default() -> Self {
        Self::new(10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mend_loads_pool() {
        let mut m = Mend::new(5.0);
        m.mend(30.0);
        assert!((m.mend_pool - 30.0).abs() < 1e-5);
        assert!(m.is_mending());
    }

    #[test]
    fn mend_stacks_multiple_calls() {
        let mut m = Mend::new(5.0);
        m.mend(20.0);
        m.mend(10.0);
        assert!((m.mend_pool - 30.0).abs() < 1e-5);
    }

    #[test]
    fn mend_no_op_when_disabled() {
        let mut m = Mend::new(5.0);
        m.enabled = false;
        m.mend(20.0);
        assert_eq!(m.mend_pool, 0.0);
    }

    #[test]
    fn mend_no_op_when_amount_zero_or_negative() {
        let mut m = Mend::new(5.0);
        m.mend(0.0);
        m.mend(-5.0);
        assert_eq!(m.mend_pool, 0.0);
    }

    #[test]
    fn tick_returns_healing_applied() {
        let mut m = Mend::new(10.0);
        m.mend(50.0);
        let healed = m.tick(1.0); // 10 HP
        assert!((healed - 10.0).abs() < 1e-5);
        assert!((m.mend_pool - 40.0).abs() < 1e-5);
    }

    #[test]
    fn tick_caps_heal_at_pool() {
        let mut m = Mend::new(100.0);
        m.mend(5.0); // less than rate*dt would give
        let healed = m.tick(1.0);
        assert!((healed - 5.0).abs() < 1e-5);
        assert_eq!(m.mend_pool, 0.0);
    }

    #[test]
    fn tick_fires_just_depleted_on_empty() {
        let mut m = Mend::new(100.0);
        m.mend(5.0);
        m.tick(1.0); // drains fully
        assert!(m.just_depleted);
        assert!(!m.is_mending());
    }

    #[test]
    fn tick_clears_just_depleted_next_frame() {
        let mut m = Mend::new(100.0);
        m.mend(5.0);
        m.tick(1.0); // depletes
        m.tick(0.016); // clears
        assert!(!m.just_depleted);
    }

    #[test]
    fn tick_no_just_depleted_when_pool_not_empty() {
        let mut m = Mend::new(5.0);
        m.mend(50.0);
        m.tick(1.0); // 5 HP healed, 45 remain
        assert!(!m.just_depleted);
        assert!(m.is_mending());
    }

    #[test]
    fn tick_returns_zero_when_pool_empty() {
        let mut m = Mend::new(10.0);
        let healed = m.tick(1.0);
        assert_eq!(healed, 0.0);
    }

    #[test]
    fn tick_returns_zero_when_disabled() {
        let mut m = Mend::new(10.0);
        m.mend(50.0);
        m.enabled = false;
        let healed = m.tick(1.0);
        assert_eq!(healed, 0.0);
        assert!((m.mend_pool - 50.0).abs() < 1e-5); // pool unchanged
    }

    #[test]
    fn multiple_ticks_drain_pool() {
        let mut m = Mend::new(10.0);
        m.mend(25.0);
        m.tick(1.0); // 10 healed, 15 remain
        m.tick(1.0); // 10 healed, 5 remain
        m.tick(1.0); // 5 healed, 0 remain
        assert_eq!(m.mend_pool, 0.0);
        assert!(m.just_depleted);
    }

    #[test]
    fn can_mend_again_after_depletion() {
        let mut m = Mend::new(10.0);
        m.mend(10.0);
        m.tick(1.0); // depletes
        m.tick(0.016); // clears flags
        m.mend(20.0);
        assert!(m.is_mending());
        assert!((m.mend_pool - 20.0).abs() < 1e-5);
    }

    #[test]
    fn rate_zero_heals_nothing_per_tick() {
        let mut m = Mend::new(0.0);
        m.mend(50.0);
        let healed = m.tick(1.0);
        assert_eq!(healed, 0.0);
        assert!((m.mend_pool - 50.0).abs() < 1e-5); // pool unchanged
    }

    #[test]
    fn is_mending_false_initially() {
        let m = Mend::new(10.0);
        assert!(!m.is_mending());
    }
}
