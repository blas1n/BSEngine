use bevy_ecs::prelude::Component;

/// Intercepts a fraction of each incoming hit before it reaches the entity's health.
///
/// Unlike `Barrier` (a separate HP pool that blocks damage chunks) or `Armor`
/// (flat/multiplicative reduction), `Absorption` is fraction-based: each hit is
/// split — `fraction` goes to this component, the remainder continues to health.
/// An optional finite `pool` limits the total absorbed; when `pool` reaches zero
/// the component deactivates and sets `just_depleted`.
///
/// `absorb(damage)` returns the remaining damage after absorption.
/// `pool == 0.0` means unlimited absorption (fraction-only, no cap).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Absorption {
    /// Fraction [0.0, 1.0] of each hit absorbed by this component.
    pub fraction: f32,
    /// Total damage remaining in the absorption pool. `0.0` = unlimited.
    pub pool: f32,
    /// Maximum pool (for UI fill fraction). `0.0` when pool is unlimited.
    pub max_pool: f32,
    /// Running total of all damage absorbed since last reset.
    pub absorbed_total: f32,
    /// True the frame the pool was depleted to zero.
    pub just_depleted: bool,
    pub enabled: bool,
}

impl Absorption {
    /// Create an unlimited-pool absorption (pure fraction).
    pub fn new(fraction: f32) -> Self {
        Self {
            fraction: fraction.clamp(0.0, 1.0),
            pool: 0.0,
            max_pool: 0.0,
            absorbed_total: 0.0,
            just_depleted: false,
            enabled: true,
        }
    }

    /// Create a finite-pool absorption.
    pub fn with_pool(mut self, pool: f32) -> Self {
        self.pool = pool.max(0.0);
        self.max_pool = self.pool;
        self
    }

    /// Absorb a portion of `damage`. Returns the unabsorbed remainder passed to health.
    ///
    /// When the pool is finite and runs out, `just_depleted` is set; any remaining
    /// damage in that hit is returned in full.
    pub fn absorb(&mut self, damage: f32) -> f32 {
        if !self.enabled || damage <= 0.0 {
            return damage;
        }

        let unlimited = self.max_pool <= 0.0;

        if !unlimited && self.pool <= 0.0 {
            return damage;
        }

        let would_absorb = damage * self.fraction;

        let actually_absorbed = if unlimited {
            would_absorb
        } else {
            let capped = would_absorb.min(self.pool);
            self.pool -= capped;
            if self.pool <= 0.0 {
                self.pool = 0.0;
                self.just_depleted = true;
            }
            capped
        };

        if unlimited {
            self.pool = 0.0; // stays at 0 (sentinel for unlimited)
        }

        self.absorbed_total += actually_absorbed;
        damage - actually_absorbed
    }

    /// Replenish the pool (for re-application by a buff system).
    pub fn refill(&mut self, amount: f32) {
        if self.max_pool > 0.0 {
            self.pool = (self.pool + amount).min(self.max_pool);
            self.just_depleted = false;
        }
    }

    /// Clear per-frame flags.
    pub fn tick(&mut self, _dt: f32) {
        self.just_depleted = false;
    }

    pub fn is_active(&self) -> bool {
        self.enabled && (self.max_pool <= 0.0 || self.pool > 0.0)
    }

    /// Pool fill fraction [0.0, 1.0]. Returns 1.0 for unlimited pools.
    pub fn pool_fraction(&self) -> f32 {
        if self.max_pool <= 0.0 {
            return 1.0;
        }
        self.pool / self.max_pool
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlimited_absorbs_fraction() {
        let mut a = Absorption::new(0.5);
        let remaining = a.absorb(100.0);
        assert!((remaining - 50.0).abs() < 1e-4);
        assert!((a.absorbed_total - 50.0).abs() < 1e-4);
    }

    #[test]
    fn finite_pool_absorbs_and_depletes() {
        let mut a = Absorption::new(0.5).with_pool(30.0);
        // First hit: would absorb 50 but pool only has 30.
        let remaining = a.absorb(100.0);
        assert!((remaining - 70.0).abs() < 1e-4); // 100 - 30
        assert!((a.pool).abs() < 1e-4);
        assert!(a.just_depleted);
    }

    #[test]
    fn finite_pool_partial_hit() {
        let mut a = Absorption::new(0.5).with_pool(100.0);
        let remaining = a.absorb(60.0); // absorbs 30
        assert!((remaining - 30.0).abs() < 1e-4);
        assert!((a.pool - 70.0).abs() < 1e-4);
        assert!(!a.just_depleted);
    }

    #[test]
    fn depleted_pool_passes_damage_through() {
        let mut a = Absorption::new(0.5).with_pool(10.0);
        a.absorb(100.0); // depletes pool
        let remaining = a.absorb(50.0);
        assert!((remaining - 50.0).abs() < 1e-4); // full damage passes
    }

    #[test]
    fn refill_replenishes_pool() {
        let mut a = Absorption::new(0.5).with_pool(100.0);
        a.absorb(200.0); // fully deplete
        a.refill(50.0);
        assert!((a.pool - 50.0).abs() < 1e-4);
        assert!(!a.just_depleted);
    }

    #[test]
    fn refill_capped_at_max() {
        let mut a = Absorption::new(0.5).with_pool(100.0);
        a.refill(200.0);
        assert!((a.pool - 100.0).abs() < 1e-4);
    }

    #[test]
    fn tick_clears_just_depleted() {
        let mut a = Absorption::new(0.5).with_pool(10.0);
        a.absorb(100.0);
        a.tick(0.016);
        assert!(!a.just_depleted);
    }

    #[test]
    fn disabled_passes_all_damage() {
        let mut a = Absorption::new(0.5);
        a.enabled = false;
        let remaining = a.absorb(100.0);
        assert!((remaining - 100.0).abs() < 1e-4);
    }

    #[test]
    fn pool_fraction_unlimited_is_one() {
        let a = Absorption::new(0.5);
        assert!((a.pool_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn pool_fraction_finite() {
        let mut a = Absorption::new(0.5).with_pool(100.0);
        a.pool = 50.0;
        assert!((a.pool_fraction() - 0.5).abs() < 1e-5);
    }
}
