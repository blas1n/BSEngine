use bevy_ecs::prelude::Component;

/// Self-repairing defensive mesh. Models a woven lattice that absorbs
/// incoming force proportional to how intact it is; sustained damage frays
/// it while respite lets it weave itself back.
///
/// `fray()` marks the mesh as under active damage; no-op if already fraying
/// or disabled.
///
/// `mend()` ends active fraying; no-op if not currently fraying.
///
/// `tick(dt)` clears both one-frame flags first, then:
/// - If `fraying`: decreases `weft_density` by `fray_rate * dt` (floored at
///   0); fires `just_rent` the first time it reaches 0.
/// - If `!fraying` and `weft_density < max_density`: increases by
///   `weave_rate * dt` (capped at `max_density`); fires `just_mended` the
///   first time it reaches the cap.
/// - No-op when disabled (flags are still cleared).
///
/// `is_intact()` returns `weft_density >= max_density && enabled`.
///
/// `density_fraction()` returns
/// `(weft_density / max_density).clamp(0.0, 1.0)`.
///
/// `effective_absorption(base)` returns `base * density_fraction()` when
/// enabled (intact mesh absorbs fully; a torn mesh absorbs nothing); returns
/// `base` unchanged otherwise.
///
/// Distinct from `Shield` (binary on/off protection), `Armor` (flat damage
/// reduction), and `Weal` (positive well-being regen): Weft models a
/// **woven mesh that degrades under sustained attack and self-repairs during
/// respite**, with absorption scaling continuously to current mesh integrity.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Weft {
    /// Current mesh density [0.0, max_density].
    pub weft_density: f32,
    /// Fully intact density. Clamped >= 1.0.
    pub max_density: f32,
    /// Self-repair rate per second while not fraying. Clamped >= 0.0.
    pub weave_rate: f32,
    /// Damage rate per second while fraying. Clamped >= 0.0.
    pub fray_rate: f32,
    pub fraying: bool,
    pub just_rent: bool,
    pub just_mended: bool,
    pub enabled: bool,
}

impl Weft {
    pub fn new(max_density: f32, weave_rate: f32, fray_rate: f32) -> Self {
        Self {
            weft_density: max_density.max(1.0),
            max_density: max_density.max(1.0),
            weave_rate: weave_rate.max(0.0),
            fray_rate: fray_rate.max(0.0),
            fraying: false,
            just_rent: false,
            just_mended: false,
            enabled: true,
        }
    }

    /// Begin fraying. No-op if already fraying or disabled.
    pub fn fray(&mut self) {
        if !self.enabled || self.fraying {
            return;
        }
        self.fraying = true;
    }

    /// End fraying. No-op if not currently fraying.
    pub fn mend(&mut self) {
        if !self.fraying {
            return;
        }
        self.fraying = false;
    }

    /// Advance one frame: clear flags, then degrade or repair mesh density.
    /// No-op (beyond flag clear) when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_rent = false;
        self.just_mended = false;

        if !self.enabled {
            return;
        }

        if self.fraying {
            let was_above = self.weft_density > 0.0;
            self.weft_density = (self.weft_density - self.fray_rate * dt).max(0.0);
            if was_above && self.weft_density <= 0.0 {
                self.just_rent = true;
            }
        } else if self.weft_density < self.max_density {
            let was_below = self.weft_density < self.max_density;
            self.weft_density = (self.weft_density + self.weave_rate * dt).min(self.max_density);
            if was_below && self.weft_density >= self.max_density {
                self.just_mended = true;
            }
        }
    }

    /// `true` when mesh is fully intact and component is enabled.
    pub fn is_intact(&self) -> bool {
        self.weft_density >= self.max_density && self.enabled
    }

    /// Mesh integrity as a fraction [0.0, 1.0].
    pub fn density_fraction(&self) -> f32 {
        (self.weft_density / self.max_density).clamp(0.0, 1.0)
    }

    /// Scale `base` absorption by mesh integrity. Returns
    /// `base * density_fraction()` when enabled; `base` otherwise.
    pub fn effective_absorption(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * self.density_fraction()
    }
}

impl Default for Weft {
    fn default() -> Self {
        Self::new(10.0, 2.0, 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Weft {
        Weft::new(10.0, 2.0, 3.0)
    }

    #[test]
    fn new_starts_intact() {
        let w = w();
        assert!((w.weft_density - 10.0).abs() < 1e-5);
        assert!(!w.fraying);
        assert!(!w.just_rent);
        assert!(!w.just_mended);
        assert!(w.is_intact());
    }

    #[test]
    fn fray_sets_fraying() {
        let mut w = w();
        w.fray();
        assert!(w.fraying);
    }

    #[test]
    fn fray_no_op_when_already_fraying() {
        let mut w = w();
        w.fray();
        w.tick(1.0); // 10 - 3 = 7
        w.fray(); // no-op
        assert!(w.fraying);
        assert!((w.weft_density - 7.0).abs() < 1e-4);
    }

    #[test]
    fn fray_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.fray();
        assert!(!w.fraying);
    }

    #[test]
    fn mend_clears_fraying() {
        let mut w = w();
        w.fray();
        w.mend();
        assert!(!w.fraying);
    }

    #[test]
    fn mend_no_op_when_not_fraying() {
        let mut w = w();
        w.mend(); // no panic
        assert!(!w.fraying);
    }

    #[test]
    fn tick_degrades_density_while_fraying() {
        let mut w = w(); // fray_rate=3.0
        w.fray();
        w.tick(1.0); // 10 - 3 = 7
        assert!((w.weft_density - 7.0).abs() < 1e-4);
    }

    #[test]
    fn tick_floors_density_at_zero() {
        let mut w = w();
        w.fray();
        w.tick(100.0); // 10 - 300 → 0
        assert_eq!(w.weft_density, 0.0);
    }

    #[test]
    fn tick_no_repair_while_fraying() {
        let mut w = w();
        w.fray();
        w.tick(1.0); // degraded
        let d = w.weft_density;
        assert!(d < 10.0);
    }

    #[test]
    fn tick_repairs_density_while_not_fraying() {
        let mut w = w(); // weave_rate=2.0
        w.fray();
        w.tick(3.0); // 10 - 9 = 1.0
        w.mend();
        w.tick(1.0); // 1.0 + 2.0 = 3.0
        assert!((w.weft_density - 3.0).abs() < 1e-4);
    }

    #[test]
    fn tick_caps_repair_at_max() {
        let mut w = w();
        w.fray();
        w.tick(1.0); // 7.0
        w.mend();
        w.tick(100.0); // capped at 10
        assert!((w.weft_density - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_no_repair_when_already_intact() {
        let mut w = w(); // starts at max
        w.tick(5.0); // already intact, no change
        assert!((w.weft_density - 10.0).abs() < 1e-5);
        assert!(!w.just_mended);
    }

    #[test]
    fn tick_no_op_when_disabled_no_fray() {
        let mut w = w();
        w.fray();
        w.enabled = false;
        w.tick(5.0);
        assert!((w.weft_density - 10.0).abs() < 1e-5);
    }

    #[test]
    fn tick_no_op_when_disabled_no_repair() {
        let mut w = w();
        w.fray();
        w.tick(3.0); // 1.0
        w.mend();
        w.enabled = false;
        w.tick(5.0); // no repair
        assert!((w.weft_density - 1.0).abs() < 1e-4);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut w = w();
        w.just_rent = true;
        w.just_mended = true;
        w.enabled = false;
        w.tick(0.016);
        assert!(!w.just_rent);
        assert!(!w.just_mended);
    }

    #[test]
    fn just_rent_fires_when_density_hits_zero() {
        let mut w = w();
        w.fray();
        w.tick(4.0); // 10 - 12 = 0 → just_rent
        assert!(w.just_rent);
    }

    #[test]
    fn just_rent_clears_next_tick() {
        let mut w = w();
        w.fray();
        w.tick(4.0); // rent
        w.tick(0.016); // clears
        assert!(!w.just_rent);
    }

    #[test]
    fn just_rent_fires_only_once_at_zero() {
        let mut w = w();
        w.fray();
        w.tick(4.0); // rent
        w.tick(0.016); // cleared
        w.tick(1.0); // stays 0, no re-fire
        assert!(!w.just_rent);
    }

    #[test]
    fn just_mended_fires_when_density_reaches_max() {
        let mut w = w(); // weave_rate=2.0
        w.fray();
        w.tick(3.0); // 1.0
        w.mend();
        w.tick(100.0); // 1.0 + 200 → cap → just_mended
        assert!(w.just_mended);
    }

    #[test]
    fn just_mended_clears_next_tick() {
        let mut w = w();
        w.fray();
        w.tick(3.0); // 1.0
        w.mend();
        w.tick(100.0); // mended
        w.tick(0.016); // clears
        assert!(!w.just_mended);
    }

    #[test]
    fn just_mended_fires_only_once_at_max() {
        let mut w = w();
        w.fray();
        w.tick(3.0); // 1.0
        w.mend();
        w.tick(100.0); // mended
        w.tick(0.016); // cleared
        w.tick(1.0); // still max, no re-fire
        assert!(!w.just_mended);
    }

    #[test]
    fn is_intact_true_at_max() {
        let w = w();
        assert!(w.is_intact());
    }

    #[test]
    fn is_intact_false_below_max() {
        let mut w = w();
        w.fray();
        w.tick(1.0); // 7.0 < 10.0
        assert!(!w.is_intact());
    }

    #[test]
    fn is_intact_false_when_disabled() {
        let mut w = w();
        w.enabled = false;
        assert!(!w.is_intact());
    }

    #[test]
    fn density_fraction_one_when_intact() {
        let w = w();
        assert!((w.density_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn density_fraction_half_at_midpoint() {
        let mut w = w();
        w.weft_density = 5.0;
        assert!((w.density_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn density_fraction_zero_when_torn() {
        let mut w = w();
        w.fray();
        w.tick(100.0);
        assert_eq!(w.density_fraction(), 0.0);
    }

    #[test]
    fn effective_absorption_full_when_intact() {
        let w = w();
        assert!((w.effective_absorption(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_absorption_half_at_half_density() {
        let mut w = w();
        w.weft_density = 5.0;
        // 100 * 0.5 = 50
        assert!((w.effective_absorption(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_absorption_zero_when_torn() {
        let mut w = w();
        w.fray();
        w.tick(100.0);
        assert!((w.effective_absorption(100.0) - 0.0).abs() < 1e-4);
    }

    #[test]
    fn effective_absorption_passthrough_when_disabled() {
        let mut w = w();
        w.fray();
        w.tick(100.0); // torn
        w.enabled = false;
        assert!((w.effective_absorption(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn max_density_clamped_to_one() {
        let w = Weft::new(0.0, 2.0, 3.0);
        assert!((w.max_density - 1.0).abs() < 1e-5);
    }

    #[test]
    fn weave_rate_clamped_to_zero() {
        let w = Weft::new(10.0, -5.0, 3.0);
        assert_eq!(w.weave_rate, 0.0);
    }

    #[test]
    fn fray_rate_clamped_to_zero() {
        let w = Weft::new(10.0, 2.0, -1.0);
        assert_eq!(w.fray_rate, 0.0);
    }

    #[test]
    fn fray_mend_fray_cycle() {
        let mut w = w(); // weave_rate=2, fray_rate=3
        w.fray();
        w.tick(1.0); // 10 - 3 = 7
        w.mend();
        w.tick(1.0); // 7 + 2 = 9
        w.fray();
        w.tick(1.0); // 9 - 3 = 6
        assert!((w.weft_density - 6.0).abs() < 1e-4);
    }
}
