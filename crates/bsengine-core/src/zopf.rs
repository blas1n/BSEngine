use bevy_ecs::prelude::Component;

/// Braid-tension tracker. `weave` builds via `braid(amount)` and tightens
/// passively at `plait_rate` per second in `tick(dt)` or unravels
/// immediately via `unravel(amount)`.
///
/// Models twisted-loaf texture meters, hairstyle tension bars,
/// rope-braiding tightness accumulators, woven-fabric density gauges,
/// knot-tying complexity trackers, plaited-cord integrity fill levels,
/// ribbon-weaving progress indicators, macrame-tension bars,
/// artisan-bread scoring trackers, or any mechanic where strands
/// are progressively interlaced into a firm, beautiful structure —
/// only to come apart when a single thread is pulled.
///
/// `braid(amount)` adds weave; fires `just_plaited` when first
/// reaching `max_weave`. No-op when disabled.
///
/// `unravel(amount)` reduces weave immediately; fires `just_unraveled`
/// when reaching 0. No-op when disabled or already unraveled.
///
/// `tick(dt)` clears both flags, then increases weave by
/// `plait_rate * dt` (capped at `max_weave`). Fires `just_plaited`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_plaited()` returns `weave >= max_weave && enabled`.
///
/// `is_unraveled()` returns `weave == 0.0` (not gated by `enabled`).
///
/// `weave_fraction()` returns `(weave / max_weave).clamp(0, 1)`.
///
/// `effective_tension(scale)` returns `scale * weave_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 4.0)` — plaits at 4 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zopf {
    pub weave: f32,
    pub max_weave: f32,
    pub plait_rate: f32,
    pub just_plaited: bool,
    pub just_unraveled: bool,
    pub enabled: bool,
}

impl Zopf {
    pub fn new(max_weave: f32, plait_rate: f32) -> Self {
        Self {
            weave: 0.0,
            max_weave: max_weave.max(0.1),
            plait_rate: plait_rate.max(0.0),
            just_plaited: false,
            just_unraveled: false,
            enabled: true,
        }
    }

    /// Add weave; fires `just_plaited` when first reaching max.
    /// No-op when disabled.
    pub fn braid(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.weave < self.max_weave;
        self.weave = (self.weave + amount).min(self.max_weave);
        if was_below && self.weave >= self.max_weave {
            self.just_plaited = true;
        }
    }

    /// Reduce weave; fires `just_unraveled` when reaching 0.
    /// No-op when disabled or already unraveled.
    pub fn unravel(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.weave <= 0.0 {
            return;
        }
        self.weave = (self.weave - amount).max(0.0);
        if self.weave <= 0.0 {
            self.just_unraveled = true;
        }
    }

    /// Clear flags, then increase weave by `plait_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_plaited = false;
        self.just_unraveled = false;
        if self.enabled && self.plait_rate > 0.0 && self.weave < self.max_weave {
            let was_below = self.weave < self.max_weave;
            self.weave = (self.weave + self.plait_rate * dt).min(self.max_weave);
            if was_below && self.weave >= self.max_weave {
                self.just_plaited = true;
            }
        }
    }

    /// `true` when weave is at maximum and component is enabled.
    pub fn is_plaited(&self) -> bool {
        self.weave >= self.max_weave && self.enabled
    }

    /// `true` when weave is 0 (not gated by `enabled`).
    pub fn is_unraveled(&self) -> bool {
        self.weave == 0.0
    }

    /// Fraction of maximum weave [0.0, 1.0].
    pub fn weave_fraction(&self) -> f32 {
        (self.weave / self.max_weave).clamp(0.0, 1.0)
    }

    /// Returns `scale * weave_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_tension(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.weave_fraction()
    }
}

impl Default for Zopf {
    fn default() -> Self {
        Self::new(100.0, 4.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zopf {
        Zopf::new(100.0, 4.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_unraveled() {
        let z = z();
        assert_eq!(z.weave, 0.0);
        assert!(z.is_unraveled());
        assert!(!z.is_plaited());
    }

    #[test]
    fn new_clamps_max_weave() {
        let z = Zopf::new(-5.0, 4.0);
        assert!((z.max_weave - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_plait_rate() {
        let z = Zopf::new(100.0, -3.0);
        assert_eq!(z.plait_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zopf::default();
        assert!((z.max_weave - 100.0).abs() < 1e-5);
        assert!((z.plait_rate - 4.0).abs() < 1e-5);
    }

    // --- braid ---

    #[test]
    fn braid_adds_weave() {
        let mut z = z();
        z.braid(40.0);
        assert!((z.weave - 40.0).abs() < 1e-3);
    }

    #[test]
    fn braid_clamps_at_max() {
        let mut z = z();
        z.braid(200.0);
        assert!((z.weave - 100.0).abs() < 1e-3);
    }

    #[test]
    fn braid_fires_just_plaited_at_max() {
        let mut z = z();
        z.braid(100.0);
        assert!(z.just_plaited);
        assert!(z.is_plaited());
    }

    #[test]
    fn braid_no_just_plaited_when_already_at_max() {
        let mut z = z();
        z.weave = 100.0;
        z.braid(10.0);
        assert!(!z.just_plaited);
    }

    #[test]
    fn braid_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.braid(50.0);
        assert_eq!(z.weave, 0.0);
    }

    #[test]
    fn braid_no_op_when_amount_zero() {
        let mut z = z();
        z.braid(0.0);
        assert_eq!(z.weave, 0.0);
    }

    // --- unravel ---

    #[test]
    fn unravel_reduces_weave() {
        let mut z = z();
        z.weave = 60.0;
        z.unravel(20.0);
        assert!((z.weave - 40.0).abs() < 1e-3);
    }

    #[test]
    fn unravel_clamps_at_zero() {
        let mut z = z();
        z.weave = 30.0;
        z.unravel(200.0);
        assert_eq!(z.weave, 0.0);
    }

    #[test]
    fn unravel_fires_just_unraveled_at_zero() {
        let mut z = z();
        z.weave = 30.0;
        z.unravel(30.0);
        assert!(z.just_unraveled);
    }

    #[test]
    fn unravel_no_op_when_already_unraveled() {
        let mut z = z();
        z.unravel(10.0);
        assert!(!z.just_unraveled);
    }

    #[test]
    fn unravel_no_op_when_disabled() {
        let mut z = z();
        z.weave = 50.0;
        z.enabled = false;
        z.unravel(50.0);
        assert!((z.weave - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_tightens_weave() {
        let mut z = z(); // rate=4
        z.tick(2.0); // 0 + 4*2 = 8
        assert!((z.weave - 8.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_plaited_on_plait_to_max() {
        let mut z = Zopf::new(100.0, 200.0);
        z.weave = 95.0;
        z.tick(1.0);
        assert!(z.just_plaited);
        assert!(z.is_plaited());
    }

    #[test]
    fn tick_no_growth_when_already_plaited() {
        let mut z = z();
        z.weave = 100.0;
        z.tick(1.0);
        assert!(!z.just_plaited);
    }

    #[test]
    fn tick_no_growth_when_rate_zero() {
        let mut z = Zopf::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.weave, 0.0);
    }

    #[test]
    fn tick_no_growth_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.weave, 0.0);
    }

    #[test]
    fn tick_clears_just_plaited() {
        let mut z = Zopf::new(100.0, 200.0);
        z.weave = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_plaited);
    }

    #[test]
    fn tick_clears_just_unraveled() {
        let mut z = z();
        z.weave = 10.0;
        z.unravel(10.0);
        z.tick(0.016);
        assert!(!z.just_unraveled);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=4
        z.tick(5.0); // 4*5 = 20
        assert!((z.weave - 20.0).abs() < 1e-3);
    }

    // --- is_plaited / is_unraveled ---

    #[test]
    fn is_plaited_false_when_disabled() {
        let mut z = z();
        z.weave = 100.0;
        z.enabled = false;
        assert!(!z.is_plaited());
    }

    #[test]
    fn is_unraveled_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_unraveled());
    }

    // --- weave_fraction / effective_tension ---

    #[test]
    fn weave_fraction_zero_when_unraveled() {
        assert_eq!(z().weave_fraction(), 0.0);
    }

    #[test]
    fn weave_fraction_half_at_midpoint() {
        let mut z = z();
        z.weave = 50.0;
        assert!((z.weave_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_tension_zero_when_unraveled() {
        assert_eq!(z().effective_tension(100.0), 0.0);
    }

    #[test]
    fn effective_tension_scales_with_weave() {
        let mut z = z();
        z.weave = 60.0;
        assert!((z.effective_tension(100.0) - 60.0).abs() < 1e-3);
    }

    #[test]
    fn effective_tension_zero_when_disabled() {
        let mut z = z();
        z.weave = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_tension(100.0), 0.0);
    }
}
