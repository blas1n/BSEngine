use bevy_ecs::prelude::Component;

/// Timber-material accumulation tracker named after wood, the noun
/// meaning the hard fibrous substance composing the greater part
/// of the trunk and branches of a tree; timber cut or sawn for use
/// as building material or fuel — from the Old English wudu, widu
/// (a wood, a forest; timber), from the Proto-Germanic widu-
/// (wood, forest), from the Proto-Indo-European root widhu-
/// (tree, wood). The root widhu- also gave the Welsh gwŷdd
/// (trees) and the Irish fiodh (wood, forest), placing wood
/// among the oldest attested vocabulary of the Indo-European
/// family — a word as old as the tools that first shaped it.
/// Before metal, stone, and ceramic, wood was the universal
/// material: it built houses, made fire, shaped tools, constructed
/// boats, and provided the handles for weapons made of everything
/// else. The variety of woods — hardwood and softwood, heartwood
/// and sapwood, green and seasoned, structural and decorative —
/// reflects the diversity of uses to which it was put. In
/// alchemical and elemental traditions, wood occupies a special
/// place: in the Chinese five-element system (wǔxíng), wood
/// (木, mù) is one of the five fundamental phases, associated
/// with spring, growth, the colour green, the east, and the
/// liver. In game mechanics, a wood mechanic models the slow
/// accumulation of timber or material stock — the filling of
/// the lumber yard, the build of the woodpile, the gather of
/// the faggot bundle that eventually reaches the threshold at
/// which construction, fire-building, or crafting becomes
/// possible. `stock` builds via `gather(amount)` and accumulates
/// passively at `grow_rate` per second in `tick(dt)` or is
/// consumed via `burn(amount)`.
///
/// Models timber-stock fill levels, lumber-saturation bars,
/// material-accumulation trackers, fuel-wood gauges, firewood-
/// fill levels, building-material saturation indicators,
/// resource-stockpile accumulation bars, kindling meters,
/// craft-material completion fill levels, or any mechanic where
/// a character, settlement, or entity slowly accumulates the
/// timber, fuel, or wooden material required to build a
/// structure, kindle a fire, craft an item, or trigger a
/// construction event — each log gathered, each branch broken,
/// each tree felled adding a fraction to the stockpile until
/// the threshold is crossed and the work can begin.
///
/// `gather(amount)` adds stock; fires `just_stocked` when first
/// reaching `max_stock`. No-op when disabled.
///
/// `burn(amount)` reduces stock immediately; fires `just_bare`
/// when reaching 0. No-op when disabled or already bare.
///
/// `tick(dt)` clears both flags, then increases stock by
/// `grow_rate * dt` (capped at `max_stock`). Fires `just_stocked`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_stocked()` returns `stock >= max_stock && enabled`.
///
/// `is_bare()` returns `stock == 0.0` (not gated by `enabled`).
///
/// `stock_fraction()` returns `(stock / max_stock).clamp(0, 1)`.
///
/// `effective_yield(scale)` returns `scale * stock_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — grows at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wood {
    pub stock: f32,
    pub max_stock: f32,
    pub grow_rate: f32,
    pub just_stocked: bool,
    pub just_bare: bool,
    pub enabled: bool,
}

impl Wood {
    pub fn new(max_stock: f32, grow_rate: f32) -> Self {
        Self {
            stock: 0.0,
            max_stock: max_stock.max(0.1),
            grow_rate: grow_rate.max(0.0),
            just_stocked: false,
            just_bare: false,
            enabled: true,
        }
    }

    /// Add stock; fires `just_stocked` when first reaching max.
    /// No-op when disabled.
    pub fn gather(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.stock < self.max_stock;
        self.stock = (self.stock + amount).min(self.max_stock);
        if was_below && self.stock >= self.max_stock {
            self.just_stocked = true;
        }
    }

    /// Reduce stock; fires `just_bare` when reaching 0.
    /// No-op when disabled or already bare.
    pub fn burn(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.stock <= 0.0 {
            return;
        }
        self.stock = (self.stock - amount).max(0.0);
        if self.stock <= 0.0 {
            self.just_bare = true;
        }
    }

    /// Clear flags, then increase stock by `grow_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_stocked = false;
        self.just_bare = false;
        if self.enabled && self.grow_rate > 0.0 && self.stock < self.max_stock {
            let was_below = self.stock < self.max_stock;
            self.stock = (self.stock + self.grow_rate * dt).min(self.max_stock);
            if was_below && self.stock >= self.max_stock {
                self.just_stocked = true;
            }
        }
    }

    /// `true` when stock is at maximum and component is enabled.
    pub fn is_stocked(&self) -> bool {
        self.stock >= self.max_stock && self.enabled
    }

    /// `true` when stock is 0 (not gated by `enabled`).
    pub fn is_bare(&self) -> bool {
        self.stock == 0.0
    }

    /// Fraction of maximum stock [0.0, 1.0].
    pub fn stock_fraction(&self) -> f32 {
        (self.stock / self.max_stock).clamp(0.0, 1.0)
    }

    /// Returns `scale * stock_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_yield(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.stock_fraction()
    }
}

impl Default for Wood {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wood {
        Wood::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_bare() {
        let w = w();
        assert_eq!(w.stock, 0.0);
        assert!(w.is_bare());
        assert!(!w.is_stocked());
    }

    #[test]
    fn new_clamps_max_stock() {
        let w = Wood::new(-5.0, 1.5);
        assert!((w.max_stock - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_grow_rate() {
        let w = Wood::new(100.0, -1.5);
        assert_eq!(w.grow_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Wood::default();
        assert!((w.max_stock - 100.0).abs() < 1e-5);
        assert!((w.grow_rate - 1.5).abs() < 1e-5);
    }

    // --- gather ---

    #[test]
    fn gather_adds_stock() {
        let mut w = w();
        w.gather(40.0);
        assert!((w.stock - 40.0).abs() < 1e-3);
    }

    #[test]
    fn gather_clamps_at_max() {
        let mut w = w();
        w.gather(200.0);
        assert!((w.stock - 100.0).abs() < 1e-3);
    }

    #[test]
    fn gather_fires_just_stocked_at_max() {
        let mut w = w();
        w.gather(100.0);
        assert!(w.just_stocked);
        assert!(w.is_stocked());
    }

    #[test]
    fn gather_no_just_stocked_when_already_at_max() {
        let mut w = w();
        w.stock = 100.0;
        w.gather(10.0);
        assert!(!w.just_stocked);
    }

    #[test]
    fn gather_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.gather(50.0);
        assert_eq!(w.stock, 0.0);
    }

    #[test]
    fn gather_no_op_when_amount_zero() {
        let mut w = w();
        w.gather(0.0);
        assert_eq!(w.stock, 0.0);
    }

    // --- burn ---

    #[test]
    fn burn_reduces_stock() {
        let mut w = w();
        w.stock = 60.0;
        w.burn(20.0);
        assert!((w.stock - 40.0).abs() < 1e-3);
    }

    #[test]
    fn burn_clamps_at_zero() {
        let mut w = w();
        w.stock = 30.0;
        w.burn(200.0);
        assert_eq!(w.stock, 0.0);
    }

    #[test]
    fn burn_fires_just_bare_at_zero() {
        let mut w = w();
        w.stock = 30.0;
        w.burn(30.0);
        assert!(w.just_bare);
    }

    #[test]
    fn burn_no_op_when_already_bare() {
        let mut w = w();
        w.burn(10.0);
        assert!(!w.just_bare);
    }

    #[test]
    fn burn_no_op_when_disabled() {
        let mut w = w();
        w.stock = 50.0;
        w.enabled = false;
        w.burn(50.0);
        assert!((w.stock - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_stock() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.stock - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_stocked_on_stock_to_max() {
        let mut w = Wood::new(100.0, 200.0);
        w.stock = 95.0;
        w.tick(1.0);
        assert!(w.just_stocked);
        assert!(w.is_stocked());
    }

    #[test]
    fn tick_no_build_when_already_stocked() {
        let mut w = w();
        w.stock = 100.0;
        w.tick(1.0);
        assert!(!w.just_stocked);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Wood::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.stock, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.stock, 0.0);
    }

    #[test]
    fn tick_clears_just_stocked() {
        let mut w = Wood::new(100.0, 200.0);
        w.stock = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_stocked);
    }

    #[test]
    fn tick_clears_just_bare() {
        let mut w = w();
        w.stock = 10.0;
        w.burn(10.0);
        w.tick(0.016);
        assert!(!w.just_bare);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.stock - 9.0).abs() < 1e-3);
    }

    // --- is_stocked / is_bare ---

    #[test]
    fn is_stocked_false_when_disabled() {
        let mut w = w();
        w.stock = 100.0;
        w.enabled = false;
        assert!(!w.is_stocked());
    }

    #[test]
    fn is_bare_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_bare());
    }

    // --- stock_fraction / effective_yield ---

    #[test]
    fn stock_fraction_zero_when_bare() {
        assert_eq!(w().stock_fraction(), 0.0);
    }

    #[test]
    fn stock_fraction_half_at_midpoint() {
        let mut w = w();
        w.stock = 50.0;
        assert!((w.stock_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_yield_zero_when_bare() {
        assert_eq!(w().effective_yield(100.0), 0.0);
    }

    #[test]
    fn effective_yield_scales_with_stock() {
        let mut w = w();
        w.stock = 75.0;
        assert!((w.effective_yield(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_yield_zero_when_disabled() {
        let mut w = w();
        w.stock = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_yield(100.0), 0.0);
    }
}
