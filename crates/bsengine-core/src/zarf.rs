use bevy_ecs::prelude::Component;

/// Thermal-insulation tracker. `insulation` builds via `wrap(amount)` and
/// thickens passively at `layer_rate` per second in `tick(dt)` or is
/// stripped immediately via `strip(amount)`.
///
/// Models heat-retention saturation bars, cup-grip security gauges,
/// thermal-barrier accumulation trackers, insulating-sleeve fill levels,
/// heat-shield layering indicators, cold-pack insulation meters,
/// ceramic-coat thickness gauges, blast-furnace lining health bars,
/// cryostat-jacket integrity trackers, or any mechanic where successive
/// layers of insulating material build around a core to protect it from
/// thermal exchange until the outermost layer is compromised and heat
/// bleeds through with a speed proportional to how much of the jacket
/// has been peeled away by circumstance or time.
///
/// `wrap(amount)` adds insulation; fires `just_insulated` when first
/// reaching `max_insulation`. No-op when disabled.
///
/// `strip(amount)` reduces insulation immediately; fires `just_bare`
/// when reaching 0. No-op when disabled or already bare.
///
/// `tick(dt)` clears both flags, then increases insulation by
/// `layer_rate * dt` (capped at `max_insulation`). Fires `just_insulated`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_insulated()` returns `insulation >= max_insulation && enabled`.
///
/// `is_bare()` returns `insulation == 0.0` (not gated by `enabled`).
///
/// `insulation_fraction()` returns `(insulation / max_insulation).clamp(0, 1)`.
///
/// `effective_retention(scale)` returns `scale * insulation_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — layers at 2 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zarf {
    pub insulation: f32,
    pub max_insulation: f32,
    pub layer_rate: f32,
    pub just_insulated: bool,
    pub just_bare: bool,
    pub enabled: bool,
}

impl Zarf {
    pub fn new(max_insulation: f32, layer_rate: f32) -> Self {
        Self {
            insulation: 0.0,
            max_insulation: max_insulation.max(0.1),
            layer_rate: layer_rate.max(0.0),
            just_insulated: false,
            just_bare: false,
            enabled: true,
        }
    }

    /// Add insulation; fires `just_insulated` when first reaching max.
    /// No-op when disabled.
    pub fn wrap(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.insulation < self.max_insulation;
        self.insulation = (self.insulation + amount).min(self.max_insulation);
        if was_below && self.insulation >= self.max_insulation {
            self.just_insulated = true;
        }
    }

    /// Reduce insulation; fires `just_bare` when reaching 0.
    /// No-op when disabled or already bare.
    pub fn strip(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.insulation <= 0.0 {
            return;
        }
        self.insulation = (self.insulation - amount).max(0.0);
        if self.insulation <= 0.0 {
            self.just_bare = true;
        }
    }

    /// Clear flags, then increase insulation by `layer_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_insulated = false;
        self.just_bare = false;
        if self.enabled && self.layer_rate > 0.0 && self.insulation < self.max_insulation {
            let was_below = self.insulation < self.max_insulation;
            self.insulation = (self.insulation + self.layer_rate * dt).min(self.max_insulation);
            if was_below && self.insulation >= self.max_insulation {
                self.just_insulated = true;
            }
        }
    }

    /// `true` when insulation is at maximum and component is enabled.
    pub fn is_insulated(&self) -> bool {
        self.insulation >= self.max_insulation && self.enabled
    }

    /// `true` when insulation is 0 (not gated by `enabled`).
    pub fn is_bare(&self) -> bool {
        self.insulation == 0.0
    }

    /// Fraction of maximum insulation [0.0, 1.0].
    pub fn insulation_fraction(&self) -> f32 {
        (self.insulation / self.max_insulation).clamp(0.0, 1.0)
    }

    /// Returns `scale * insulation_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_retention(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.insulation_fraction()
    }
}

impl Default for Zarf {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zarf {
        Zarf::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_bare() {
        let z = z();
        assert_eq!(z.insulation, 0.0);
        assert!(z.is_bare());
        assert!(!z.is_insulated());
    }

    #[test]
    fn new_clamps_max_insulation() {
        let z = Zarf::new(-5.0, 2.0);
        assert!((z.max_insulation - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_layer_rate() {
        let z = Zarf::new(100.0, -2.0);
        assert_eq!(z.layer_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zarf::default();
        assert!((z.max_insulation - 100.0).abs() < 1e-5);
        assert!((z.layer_rate - 2.0).abs() < 1e-5);
    }

    // --- wrap ---

    #[test]
    fn wrap_adds_insulation() {
        let mut z = z();
        z.wrap(40.0);
        assert!((z.insulation - 40.0).abs() < 1e-3);
    }

    #[test]
    fn wrap_clamps_at_max() {
        let mut z = z();
        z.wrap(200.0);
        assert!((z.insulation - 100.0).abs() < 1e-3);
    }

    #[test]
    fn wrap_fires_just_insulated_at_max() {
        let mut z = z();
        z.wrap(100.0);
        assert!(z.just_insulated);
        assert!(z.is_insulated());
    }

    #[test]
    fn wrap_no_just_insulated_when_already_at_max() {
        let mut z = z();
        z.insulation = 100.0;
        z.wrap(10.0);
        assert!(!z.just_insulated);
    }

    #[test]
    fn wrap_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.wrap(50.0);
        assert_eq!(z.insulation, 0.0);
    }

    #[test]
    fn wrap_no_op_when_amount_zero() {
        let mut z = z();
        z.wrap(0.0);
        assert_eq!(z.insulation, 0.0);
    }

    // --- strip ---

    #[test]
    fn strip_reduces_insulation() {
        let mut z = z();
        z.insulation = 60.0;
        z.strip(20.0);
        assert!((z.insulation - 40.0).abs() < 1e-3);
    }

    #[test]
    fn strip_clamps_at_zero() {
        let mut z = z();
        z.insulation = 30.0;
        z.strip(200.0);
        assert_eq!(z.insulation, 0.0);
    }

    #[test]
    fn strip_fires_just_bare_at_zero() {
        let mut z = z();
        z.insulation = 30.0;
        z.strip(30.0);
        assert!(z.just_bare);
    }

    #[test]
    fn strip_no_op_when_already_bare() {
        let mut z = z();
        z.strip(10.0);
        assert!(!z.just_bare);
    }

    #[test]
    fn strip_no_op_when_disabled() {
        let mut z = z();
        z.insulation = 50.0;
        z.enabled = false;
        z.strip(50.0);
        assert!((z.insulation - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_layers_insulation() {
        let mut z = z(); // rate=2
        z.tick(3.0); // 0 + 2*3 = 6
        assert!((z.insulation - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_insulated_on_layer_to_max() {
        let mut z = Zarf::new(100.0, 200.0);
        z.insulation = 95.0;
        z.tick(1.0);
        assert!(z.just_insulated);
        assert!(z.is_insulated());
    }

    #[test]
    fn tick_no_layer_when_already_insulated() {
        let mut z = z();
        z.insulation = 100.0;
        z.tick(1.0);
        assert!(!z.just_insulated);
    }

    #[test]
    fn tick_no_layer_when_rate_zero() {
        let mut z = Zarf::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.insulation, 0.0);
    }

    #[test]
    fn tick_no_layer_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.insulation, 0.0);
    }

    #[test]
    fn tick_clears_just_insulated() {
        let mut z = Zarf::new(100.0, 200.0);
        z.insulation = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_insulated);
    }

    #[test]
    fn tick_clears_just_bare() {
        let mut z = z();
        z.insulation = 10.0;
        z.strip(10.0);
        z.tick(0.016);
        assert!(!z.just_bare);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2
        z.tick(5.0); // 2*5 = 10
        assert!((z.insulation - 10.0).abs() < 1e-3);
    }

    // --- is_insulated / is_bare ---

    #[test]
    fn is_insulated_false_when_disabled() {
        let mut z = z();
        z.insulation = 100.0;
        z.enabled = false;
        assert!(!z.is_insulated());
    }

    #[test]
    fn is_bare_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_bare());
    }

    // --- insulation_fraction / effective_retention ---

    #[test]
    fn insulation_fraction_zero_when_bare() {
        assert_eq!(z().insulation_fraction(), 0.0);
    }

    #[test]
    fn insulation_fraction_half_at_midpoint() {
        let mut z = z();
        z.insulation = 50.0;
        assert!((z.insulation_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_retention_zero_when_bare() {
        assert_eq!(z().effective_retention(100.0), 0.0);
    }

    #[test]
    fn effective_retention_scales_with_insulation() {
        let mut z = z();
        z.insulation = 75.0;
        assert!((z.effective_retention(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_retention_zero_when_disabled() {
        let mut z = z();
        z.insulation = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_retention(100.0), 0.0);
    }
}
