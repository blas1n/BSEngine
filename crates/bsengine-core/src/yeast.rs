use bevy_ecs::prelude::Component;

/// Exponential-growth accumulator. `quantity` starts at 0 and grows
/// proportionally to its current value each tick: `quantity *= 1 + growth_rate * dt`.
/// This multiplicative model is distinct from linear accumulators — small
/// seeds grow slowly at first, then accelerate once quantity is substantial.
///
/// Models fermentation progress, viral / population spread, fungal bloom,
/// chain-reaction escalation, or any mechanic where growth compounds on
/// existing quantity.
///
/// `inoculate(amount)` seeds or adds to quantity (capped at `max_quantity`).
/// Fires `just_peaked` on first reaching max. No-op when disabled.
///
/// `sterilize(amount)` reduces quantity. Fires `just_dormant` when reaching 0.
/// No-op when disabled.
///
/// `tick(dt)` clears `just_peaked` and `just_dormant`. Then (when enabled,
/// `growth_rate > 0`, and `quantity > 0`) grows quantity multiplicatively:
/// `quantity = (quantity * (1 + growth_rate * dt)).min(max_quantity)`.
/// Fires `just_peaked` if quantity reaches max.
///
/// `is_peaked()` returns `quantity >= max_quantity && enabled`.
///
/// `is_dormant()` returns `quantity == 0.0` (not gated by `enabled`).
///
/// `quantity_fraction()` returns `(quantity / max_quantity).clamp(0, 1)`.
///
/// `effective_spread(base)` returns `base * quantity_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 0.1)` — 10% growth per second, starts dormant.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yeast {
    pub quantity: f32,
    pub max_quantity: f32,
    pub growth_rate: f32,
    pub just_peaked: bool,
    pub just_dormant: bool,
    pub enabled: bool,
}

impl Yeast {
    pub fn new(max_quantity: f32, growth_rate: f32) -> Self {
        Self {
            quantity: 0.0,
            max_quantity: max_quantity.max(0.1),
            growth_rate: growth_rate.max(0.0),
            just_peaked: false,
            just_dormant: false,
            enabled: true,
        }
    }

    /// Seed or add to quantity; fires `just_peaked` on first reaching max.
    /// No-op when disabled or already at max.
    pub fn inoculate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.quantity >= self.max_quantity {
            return;
        }
        self.quantity = (self.quantity + amount).min(self.max_quantity);
        if self.quantity >= self.max_quantity {
            self.just_peaked = true;
        }
    }

    /// Reduce quantity; fires `just_dormant` when reaching 0.
    /// No-op when disabled or quantity already 0.
    pub fn sterilize(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.quantity <= 0.0 {
            return;
        }
        self.quantity = (self.quantity - amount).max(0.0);
        if self.quantity <= 0.0 {
            self.just_dormant = true;
        }
    }

    /// Advance one frame: clear flags, then multiply quantity by
    /// `(1 + growth_rate * dt)` when enabled and quantity > 0.
    /// Fires `just_peaked` if quantity reaches `max_quantity`.
    pub fn tick(&mut self, dt: f32) {
        self.just_peaked = false;
        self.just_dormant = false;
        if self.enabled && self.growth_rate > 0.0 && self.quantity > 0.0 {
            let prev_below_max = self.quantity < self.max_quantity;
            self.quantity = (self.quantity * (1.0 + self.growth_rate * dt)).min(self.max_quantity);
            if prev_below_max && self.quantity >= self.max_quantity {
                self.just_peaked = true;
            }
        }
    }

    /// `true` when quantity is at maximum and component is enabled.
    pub fn is_peaked(&self) -> bool {
        self.quantity >= self.max_quantity && self.enabled
    }

    /// `true` when quantity is 0 (not gated by `enabled`).
    pub fn is_dormant(&self) -> bool {
        self.quantity == 0.0
    }

    /// Fraction of maximum quantity [0.0, 1.0].
    pub fn quantity_fraction(&self) -> f32 {
        (self.quantity / self.max_quantity).clamp(0.0, 1.0)
    }

    /// Returns `base * quantity_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_spread(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.quantity_fraction()
    }
}

impl Default for Yeast {
    fn default() -> Self {
        Self::new(100.0, 0.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yeast {
        Yeast::new(100.0, 0.1) // 10% growth per second
    }

    // --- construction ---

    #[test]
    fn new_starts_dormant() {
        let y = y();
        assert_eq!(y.quantity, 0.0);
        assert!(y.is_dormant());
        assert!(!y.is_peaked());
    }

    #[test]
    fn new_clamps_max_quantity() {
        let y = Yeast::new(-5.0, 0.1);
        assert!((y.max_quantity - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_growth_rate() {
        let y = Yeast::new(100.0, -0.5);
        assert_eq!(y.growth_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let y = Yeast::default();
        assert!((y.max_quantity - 100.0).abs() < 1e-5);
        assert!((y.growth_rate - 0.1).abs() < 1e-5);
        assert_eq!(y.quantity, 0.0);
    }

    // --- inoculate ---

    #[test]
    fn inoculate_seeds_quantity() {
        let mut y = y();
        y.inoculate(10.0);
        assert!((y.quantity - 10.0).abs() < 1e-4);
    }

    #[test]
    fn inoculate_clamps_at_max() {
        let mut y = y();
        y.inoculate(200.0);
        assert!((y.quantity - 100.0).abs() < 1e-5);
    }

    #[test]
    fn inoculate_fires_just_peaked_at_max() {
        let mut y = y();
        y.inoculate(100.0);
        assert!(y.just_peaked);
        assert!(y.is_peaked());
    }

    #[test]
    fn inoculate_no_op_when_at_max() {
        let mut y = y();
        y.inoculate(100.0); // peaked
        y.inoculate(10.0); // already at max
        assert!(y.just_peaked);
    }

    #[test]
    fn inoculate_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.inoculate(50.0);
        assert_eq!(y.quantity, 0.0);
    }

    #[test]
    fn inoculate_no_op_for_zero() {
        let mut y = y();
        y.inoculate(0.0);
        assert_eq!(y.quantity, 0.0);
    }

    // --- sterilize ---

    #[test]
    fn sterilize_reduces_quantity() {
        let mut y = y();
        y.inoculate(60.0);
        y.sterilize(20.0);
        assert!((y.quantity - 40.0).abs() < 1e-3);
    }

    #[test]
    fn sterilize_clamps_at_zero() {
        let mut y = y();
        y.inoculate(30.0);
        y.sterilize(200.0);
        assert_eq!(y.quantity, 0.0);
    }

    #[test]
    fn sterilize_fires_just_dormant_at_zero() {
        let mut y = y();
        y.inoculate(30.0);
        y.sterilize(30.0);
        assert!(y.just_dormant);
        assert!(y.is_dormant());
    }

    #[test]
    fn sterilize_no_op_when_already_dormant() {
        let mut y = y();
        y.sterilize(10.0); // already 0
        assert!(!y.just_dormant);
    }

    #[test]
    fn sterilize_no_op_when_disabled() {
        let mut y = y();
        y.inoculate(50.0);
        y.enabled = false;
        y.sterilize(50.0);
        assert!((y.quantity - 50.0).abs() < 1e-3);
    }

    // --- tick (multiplicative growth) ---

    #[test]
    fn tick_grows_quantity_multiplicatively() {
        let mut y = y(); // growth_rate = 0.1
        y.inoculate(50.0);
        y.tick(1.0); // 50 * 1.1 = 55
        assert!((y.quantity - 55.0).abs() < 1e-3);
    }

    #[test]
    fn tick_growth_is_proportional_to_quantity() {
        let mut y = y();
        y.inoculate(10.0); // small seed
        y.tick(1.0); // 10 * 1.1 = 11
        let small_growth = y.quantity - 10.0;

        let mut y2 = Yeast::new(100.0, 0.1);
        y2.inoculate(50.0); // larger seed
        y2.tick(1.0); // 50 * 1.1 = 55
        let large_growth = y2.quantity - 50.0;

        // larger quantity grows faster in absolute terms
        assert!(large_growth > small_growth);
    }

    #[test]
    fn tick_clamps_at_max() {
        let mut y = y();
        y.inoculate(95.0);
        y.tick(100.0); // grows far past max
        assert!((y.quantity - 100.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_peaked_when_reaching_max() {
        let mut y = y();
        y.inoculate(99.0);
        y.tick(10.0); // should exceed max
        assert!(y.just_peaked);
    }

    #[test]
    fn tick_no_growth_when_dormant() {
        let mut y = y();
        y.tick(100.0); // quantity=0, no growth
        assert_eq!(y.quantity, 0.0);
    }

    #[test]
    fn tick_no_growth_when_rate_zero() {
        let mut y = Yeast::new(100.0, 0.0);
        y.inoculate(50.0);
        y.tick(100.0);
        assert!((y.quantity - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_growth_when_disabled() {
        let mut y = y();
        y.inoculate(50.0);
        y.enabled = false;
        y.tick(1.0);
        assert!((y.quantity - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_peaked() {
        let mut y = y();
        y.inoculate(100.0);
        y.tick(0.016);
        assert!(!y.just_peaked);
    }

    #[test]
    fn tick_clears_just_dormant() {
        let mut y = y();
        y.inoculate(30.0);
        y.sterilize(30.0); // just_dormant fires
        y.tick(0.016); // cleared
        assert!(!y.just_dormant);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut y = y();
        y.inoculate(50.0);
        y.tick(2.0); // 50 * 1.2 = 60
        assert!((y.quantity - 60.0).abs() < 1e-2);
    }

    #[test]
    fn tick_no_peak_refire_when_already_peaked() {
        let mut y = y();
        y.inoculate(100.0); // just_peaked
        y.tick(0.016); // clears and growth is no-op (already at max; prev_below_max=false)
        assert!(!y.just_peaked);
    }

    // --- is_peaked / is_dormant ---

    #[test]
    fn is_peaked_false_below_max() {
        let mut y = y();
        y.inoculate(50.0);
        assert!(!y.is_peaked());
    }

    #[test]
    fn is_peaked_false_when_disabled() {
        let mut y = y();
        y.inoculate(100.0);
        y.enabled = false;
        assert!(!y.is_peaked());
    }

    #[test]
    fn is_dormant_true_at_start() {
        assert!(y().is_dormant());
    }

    #[test]
    fn is_dormant_not_gated_by_enabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_dormant());
    }

    // --- fractions / effective ---

    #[test]
    fn quantity_fraction_zero_when_dormant() {
        assert_eq!(y().quantity_fraction(), 0.0);
    }

    #[test]
    fn quantity_fraction_half_at_midpoint() {
        let mut y = y();
        y.inoculate(50.0);
        assert!((y.quantity_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_spread_zero_when_dormant() {
        assert_eq!(y().effective_spread(100.0), 0.0);
    }

    #[test]
    fn effective_spread_scales_with_fraction() {
        let mut y = y();
        y.inoculate(75.0);
        assert!((y.effective_spread(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_spread_zero_when_disabled() {
        let mut y = y();
        y.inoculate(50.0);
        y.enabled = false;
        assert_eq!(y.effective_spread(100.0), 0.0);
    }
}
