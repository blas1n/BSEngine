use bevy_ecs::prelude::Component;

/// Gold-coin wealth tracker. `gold` builds via `mint(amount)` and
/// accumulates passively at `earn_rate` per second in `tick(dt)` or
/// is spent immediately via `spend(amount)`.
///
/// Models historic gold-coin hoard meters, treasure-vault fill levels,
/// ducal-treasury accumulation bars, merchant-wealth progress gauges,
/// auction-house bid-pool trackers, pirate-plunder saturation indicators,
/// medieval-tithe collection fill levels, or any mechanic where accumulated
/// gold wealth reaches a threshold that unlocks power or prestige.
///
/// `mint(amount)` adds gold; fires `just_wealthy` when first reaching
/// `max_gold`. No-op when disabled.
///
/// `spend(amount)` reduces gold immediately; fires `just_bankrupt` when
/// reaching 0. No-op when disabled or already bankrupt.
///
/// `tick(dt)` clears both flags, then increases gold by
/// `earn_rate * dt` (capped at `max_gold`). Fires `just_wealthy`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_wealthy()` returns `gold >= max_gold && enabled`.
///
/// `is_bankrupt()` returns `gold == 0.0` (not gated by `enabled`).
///
/// `gold_fraction()` returns `(gold / max_gold).clamp(0, 1)`.
///
/// `effective_prestige(scale)` returns `scale * gold_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.0)` — earns at 1 unit/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zechin {
    pub gold: f32,
    pub max_gold: f32,
    pub earn_rate: f32,
    pub just_wealthy: bool,
    pub just_bankrupt: bool,
    pub enabled: bool,
}

impl Zechin {
    pub fn new(max_gold: f32, earn_rate: f32) -> Self {
        Self {
            gold: 0.0,
            max_gold: max_gold.max(0.1),
            earn_rate: earn_rate.max(0.0),
            just_wealthy: false,
            just_bankrupt: false,
            enabled: true,
        }
    }

    /// Add gold; fires `just_wealthy` when first reaching max.
    /// No-op when disabled.
    pub fn mint(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.gold < self.max_gold;
        self.gold = (self.gold + amount).min(self.max_gold);
        if was_below && self.gold >= self.max_gold {
            self.just_wealthy = true;
        }
    }

    /// Reduce gold; fires `just_bankrupt` when reaching 0.
    /// No-op when disabled or already bankrupt.
    pub fn spend(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.gold <= 0.0 {
            return;
        }
        self.gold = (self.gold - amount).max(0.0);
        if self.gold <= 0.0 {
            self.just_bankrupt = true;
        }
    }

    /// Clear flags, then increase gold by `earn_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_wealthy = false;
        self.just_bankrupt = false;
        if self.enabled && self.earn_rate > 0.0 && self.gold < self.max_gold {
            let was_below = self.gold < self.max_gold;
            self.gold = (self.gold + self.earn_rate * dt).min(self.max_gold);
            if was_below && self.gold >= self.max_gold {
                self.just_wealthy = true;
            }
        }
    }

    /// `true` when gold is at maximum and component is enabled.
    pub fn is_wealthy(&self) -> bool {
        self.gold >= self.max_gold && self.enabled
    }

    /// `true` when gold is 0 (not gated by `enabled`).
    pub fn is_bankrupt(&self) -> bool {
        self.gold == 0.0
    }

    /// Fraction of maximum gold [0.0, 1.0].
    pub fn gold_fraction(&self) -> f32 {
        (self.gold / self.max_gold).clamp(0.0, 1.0)
    }

    /// Returns `scale * gold_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_prestige(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.gold_fraction()
    }
}

impl Default for Zechin {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zechin {
        Zechin::new(100.0, 1.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_bankrupt() {
        let z = z();
        assert_eq!(z.gold, 0.0);
        assert!(z.is_bankrupt());
        assert!(!z.is_wealthy());
    }

    #[test]
    fn new_clamps_max_gold() {
        let z = Zechin::new(-5.0, 1.0);
        assert!((z.max_gold - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_earn_rate() {
        let z = Zechin::new(100.0, -3.0);
        assert_eq!(z.earn_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zechin::default();
        assert!((z.max_gold - 100.0).abs() < 1e-5);
        assert!((z.earn_rate - 1.0).abs() < 1e-5);
    }

    // --- mint ---

    #[test]
    fn mint_adds_gold() {
        let mut z = z();
        z.mint(40.0);
        assert!((z.gold - 40.0).abs() < 1e-3);
    }

    #[test]
    fn mint_clamps_at_max() {
        let mut z = z();
        z.mint(200.0);
        assert!((z.gold - 100.0).abs() < 1e-3);
    }

    #[test]
    fn mint_fires_just_wealthy_at_max() {
        let mut z = z();
        z.mint(100.0);
        assert!(z.just_wealthy);
        assert!(z.is_wealthy());
    }

    #[test]
    fn mint_no_just_wealthy_when_already_at_max() {
        let mut z = z();
        z.gold = 100.0;
        z.mint(10.0);
        assert!(!z.just_wealthy);
    }

    #[test]
    fn mint_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.mint(50.0);
        assert_eq!(z.gold, 0.0);
    }

    #[test]
    fn mint_no_op_when_amount_zero() {
        let mut z = z();
        z.mint(0.0);
        assert_eq!(z.gold, 0.0);
    }

    // --- spend ---

    #[test]
    fn spend_reduces_gold() {
        let mut z = z();
        z.gold = 60.0;
        z.spend(20.0);
        assert!((z.gold - 40.0).abs() < 1e-3);
    }

    #[test]
    fn spend_clamps_at_zero() {
        let mut z = z();
        z.gold = 30.0;
        z.spend(200.0);
        assert_eq!(z.gold, 0.0);
    }

    #[test]
    fn spend_fires_just_bankrupt_at_zero() {
        let mut z = z();
        z.gold = 30.0;
        z.spend(30.0);
        assert!(z.just_bankrupt);
    }

    #[test]
    fn spend_no_op_when_already_bankrupt() {
        let mut z = z();
        z.spend(10.0);
        assert!(!z.just_bankrupt);
    }

    #[test]
    fn spend_no_op_when_disabled() {
        let mut z = z();
        z.gold = 50.0;
        z.enabled = false;
        z.spend(50.0);
        assert!((z.gold - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_earns_gold() {
        let mut z = z(); // rate=1
        z.tick(1.0); // 0 + 1 = 1
        assert!((z.gold - 1.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_wealthy_on_earn_to_max() {
        let mut z = Zechin::new(100.0, 200.0);
        z.gold = 95.0;
        z.tick(1.0);
        assert!(z.just_wealthy);
        assert!(z.is_wealthy());
    }

    #[test]
    fn tick_no_earn_when_already_wealthy() {
        let mut z = z();
        z.gold = 100.0;
        z.tick(1.0);
        assert!(!z.just_wealthy);
    }

    #[test]
    fn tick_no_earn_when_rate_zero() {
        let mut z = Zechin::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.gold, 0.0);
    }

    #[test]
    fn tick_no_earn_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.gold, 0.0);
    }

    #[test]
    fn tick_clears_just_wealthy() {
        let mut z = Zechin::new(100.0, 200.0);
        z.gold = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_wealthy);
    }

    #[test]
    fn tick_clears_just_bankrupt() {
        let mut z = z();
        z.gold = 10.0;
        z.spend(10.0);
        z.tick(0.016);
        assert!(!z.just_bankrupt);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1
        z.tick(5.0); // 1*5 = 5
        assert!((z.gold - 5.0).abs() < 1e-3);
    }

    // --- is_wealthy / is_bankrupt ---

    #[test]
    fn is_wealthy_false_when_disabled() {
        let mut z = z();
        z.gold = 100.0;
        z.enabled = false;
        assert!(!z.is_wealthy());
    }

    #[test]
    fn is_bankrupt_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_bankrupt());
    }

    // --- gold_fraction / effective_prestige ---

    #[test]
    fn gold_fraction_zero_when_bankrupt() {
        assert_eq!(z().gold_fraction(), 0.0);
    }

    #[test]
    fn gold_fraction_half_at_midpoint() {
        let mut z = z();
        z.gold = 50.0;
        assert!((z.gold_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_prestige_zero_when_bankrupt() {
        assert_eq!(z().effective_prestige(100.0), 0.0);
    }

    #[test]
    fn effective_prestige_scales_with_gold() {
        let mut z = z();
        z.gold = 75.0;
        assert!((z.effective_prestige(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_prestige_zero_when_disabled() {
        let mut z = z();
        z.gold = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_prestige(100.0), 0.0);
    }
}
