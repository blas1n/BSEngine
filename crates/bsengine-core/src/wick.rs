use bevy_ecs::prelude::Component;

/// Capillary-fuel accumulation tracker named after wick, the
/// noun meaning a loosely interlaced or twisted fiber or strip
/// forming the central element of a candle, oil lamp, or similar
/// device by which the fuel is drawn up to the flame by capillary
/// action; the mechanism by which combustion is sustained and
/// regulated — from the Old English wēoce (wick of a candle),
/// from the Proto-Germanic weukō (wick), possibly from the Proto-
/// Indo-European root weik- or weig- (to be lively, to bend,
/// to move quickly). The wick is the unsung structural element
/// of the candle: without the flame, it is just twisted fiber;
/// without the wax, it is just a wick with nothing to draw;
/// without the wick, the fuel has no path from reserve to flame.
/// The wick is the interface between the passive fuel and the
/// active combustion, the capillary pathway that keeps a flame
/// going not by burning itself but by continuously offering
/// fuel to what burns. A candle wick that has too much char
/// on its tip draws fuel inefficiently and produces a smoky,
/// unsteady flame; a wick that is too thin runs dry too fast;
/// a wick that is too thick draws more fuel than can be
/// combusted and drowns the flame in excess. In extended
/// metaphorical use, a wick can be anything that serves as
/// the conduit between a reserve and an active process —
/// the mechanism by which stored potential becomes active
/// energy. In game mechanics, a wick mechanic models the
/// accumulation of capillary fuel — the saturation of the
/// wick that eventually allows a sustained flame to burn.
/// `fuel` builds via `soak(amount)` and accumulates passively
/// at `draw_rate` per second in `tick(dt)` or is expended
/// via `spend(amount)`.
///
/// Models capillary-fuel fill levels, wicking-saturation
/// bars, draw-accumulation trackers, flame-ready gauges,
/// oil-saturation fill levels, combustion-readiness indicators,
/// fuel-saturation accumulation bars, capillary meters,
/// sustained-burn fill levels, or any mechanic where a device,
/// entity, or system slowly accumulates the saturated fuel
/// or combustible material required to maintain a sustained
/// flame, trigger a fire-based effect, or reach the threshold
/// of full combustion readiness.
///
/// `soak(amount)` adds fuel; fires `just_lit` when first
/// reaching `max_fuel`. No-op when disabled.
///
/// `spend(amount)` reduces fuel immediately; fires `just_dry`
/// when reaching 0. No-op when disabled or already dry.
///
/// `tick(dt)` clears both flags, then increases fuel by
/// `draw_rate * dt` (capped at `max_fuel`). Fires `just_lit`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_lit()` returns `fuel >= max_fuel && enabled`.
///
/// `is_dry()` returns `fuel == 0.0` (not gated by `enabled`).
///
/// `fuel_fraction()` returns `(fuel / max_fuel).clamp(0, 1)`.
///
/// `effective_flame(scale)` returns `scale * fuel_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — draws at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wick {
    pub fuel: f32,
    pub max_fuel: f32,
    pub draw_rate: f32,
    pub just_lit: bool,
    pub just_dry: bool,
    pub enabled: bool,
}

impl Wick {
    pub fn new(max_fuel: f32, draw_rate: f32) -> Self {
        Self {
            fuel: 0.0,
            max_fuel: max_fuel.max(0.1),
            draw_rate: draw_rate.max(0.0),
            just_lit: false,
            just_dry: false,
            enabled: true,
        }
    }

    /// Add fuel; fires `just_lit` when first reaching max.
    /// No-op when disabled.
    pub fn soak(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.fuel < self.max_fuel;
        self.fuel = (self.fuel + amount).min(self.max_fuel);
        if was_below && self.fuel >= self.max_fuel {
            self.just_lit = true;
        }
    }

    /// Reduce fuel; fires `just_dry` when reaching 0.
    /// No-op when disabled or already dry.
    pub fn spend(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.fuel <= 0.0 {
            return;
        }
        self.fuel = (self.fuel - amount).max(0.0);
        if self.fuel <= 0.0 {
            self.just_dry = true;
        }
    }

    /// Clear flags, then increase fuel by `draw_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_lit = false;
        self.just_dry = false;
        if self.enabled && self.draw_rate > 0.0 && self.fuel < self.max_fuel {
            let was_below = self.fuel < self.max_fuel;
            self.fuel = (self.fuel + self.draw_rate * dt).min(self.max_fuel);
            if was_below && self.fuel >= self.max_fuel {
                self.just_lit = true;
            }
        }
    }

    /// `true` when fuel is at maximum and component is enabled.
    pub fn is_lit(&self) -> bool {
        self.fuel >= self.max_fuel && self.enabled
    }

    /// `true` when fuel is 0 (not gated by `enabled`).
    pub fn is_dry(&self) -> bool {
        self.fuel == 0.0
    }

    /// Fraction of maximum fuel [0.0, 1.0].
    pub fn fuel_fraction(&self) -> f32 {
        (self.fuel / self.max_fuel).clamp(0.0, 1.0)
    }

    /// Returns `scale * fuel_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_flame(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.fuel_fraction()
    }
}

impl Default for Wick {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wick {
        Wick::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_dry() {
        let w = w();
        assert_eq!(w.fuel, 0.0);
        assert!(w.is_dry());
        assert!(!w.is_lit());
    }

    #[test]
    fn new_clamps_max_fuel() {
        let w = Wick::new(-5.0, 1.5);
        assert!((w.max_fuel - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_draw_rate() {
        let w = Wick::new(100.0, -1.5);
        assert_eq!(w.draw_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Wick::default();
        assert!((w.max_fuel - 100.0).abs() < 1e-5);
        assert!((w.draw_rate - 1.5).abs() < 1e-5);
    }

    // --- soak ---

    #[test]
    fn soak_adds_fuel() {
        let mut w = w();
        w.soak(40.0);
        assert!((w.fuel - 40.0).abs() < 1e-3);
    }

    #[test]
    fn soak_clamps_at_max() {
        let mut w = w();
        w.soak(200.0);
        assert!((w.fuel - 100.0).abs() < 1e-3);
    }

    #[test]
    fn soak_fires_just_lit_at_max() {
        let mut w = w();
        w.soak(100.0);
        assert!(w.just_lit);
        assert!(w.is_lit());
    }

    #[test]
    fn soak_no_just_lit_when_already_at_max() {
        let mut w = w();
        w.fuel = 100.0;
        w.soak(10.0);
        assert!(!w.just_lit);
    }

    #[test]
    fn soak_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.soak(50.0);
        assert_eq!(w.fuel, 0.0);
    }

    #[test]
    fn soak_no_op_when_amount_zero() {
        let mut w = w();
        w.soak(0.0);
        assert_eq!(w.fuel, 0.0);
    }

    // --- spend ---

    #[test]
    fn spend_reduces_fuel() {
        let mut w = w();
        w.fuel = 60.0;
        w.spend(20.0);
        assert!((w.fuel - 40.0).abs() < 1e-3);
    }

    #[test]
    fn spend_clamps_at_zero() {
        let mut w = w();
        w.fuel = 30.0;
        w.spend(200.0);
        assert_eq!(w.fuel, 0.0);
    }

    #[test]
    fn spend_fires_just_dry_at_zero() {
        let mut w = w();
        w.fuel = 30.0;
        w.spend(30.0);
        assert!(w.just_dry);
    }

    #[test]
    fn spend_no_op_when_already_dry() {
        let mut w = w();
        w.spend(10.0);
        assert!(!w.just_dry);
    }

    #[test]
    fn spend_no_op_when_disabled() {
        let mut w = w();
        w.fuel = 50.0;
        w.enabled = false;
        w.spend(50.0);
        assert!((w.fuel - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_fuel() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.fuel - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_lit_on_fuel_to_max() {
        let mut w = Wick::new(100.0, 200.0);
        w.fuel = 95.0;
        w.tick(1.0);
        assert!(w.just_lit);
        assert!(w.is_lit());
    }

    #[test]
    fn tick_no_build_when_already_lit() {
        let mut w = w();
        w.fuel = 100.0;
        w.tick(1.0);
        assert!(!w.just_lit);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Wick::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.fuel, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.fuel, 0.0);
    }

    #[test]
    fn tick_clears_just_lit() {
        let mut w = Wick::new(100.0, 200.0);
        w.fuel = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_lit);
    }

    #[test]
    fn tick_clears_just_dry() {
        let mut w = w();
        w.fuel = 10.0;
        w.spend(10.0);
        w.tick(0.016);
        assert!(!w.just_dry);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.fuel - 9.0).abs() < 1e-3);
    }

    // --- is_lit / is_dry ---

    #[test]
    fn is_lit_false_when_disabled() {
        let mut w = w();
        w.fuel = 100.0;
        w.enabled = false;
        assert!(!w.is_lit());
    }

    #[test]
    fn is_dry_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_dry());
    }

    // --- fuel_fraction / effective_flame ---

    #[test]
    fn fuel_fraction_zero_when_dry() {
        assert_eq!(w().fuel_fraction(), 0.0);
    }

    #[test]
    fn fuel_fraction_half_at_midpoint() {
        let mut w = w();
        w.fuel = 50.0;
        assert!((w.fuel_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_flame_zero_when_dry() {
        assert_eq!(w().effective_flame(100.0), 0.0);
    }

    #[test]
    fn effective_flame_scales_with_fuel() {
        let mut w = w();
        w.fuel = 75.0;
        assert!((w.effective_flame(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_flame_zero_when_disabled() {
        let mut w = w();
        w.fuel = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_flame(100.0), 0.0);
    }
}
