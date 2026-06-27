use bevy_ecs::prelude::Component;

/// Infection-spread tracker modeled on the classical zymotic
/// theory of disease. `titer` builds via `inoculate(amount)` and
/// spreads passively at `spread_rate` per second in `tick(dt)` or
/// is suppressed immediately via `suppress(amount)`.
///
/// Models pathogen-load progression bars, epidemic-spread fill
/// levels, colony-infection saturation trackers, plague-intensity
/// accumulation gauges, bacteremia-concentration meters,
/// fermentation-spoilage progress indicators, rot-cascade intensity
/// bars, blight-spread saturation trackers, disease-endemic
/// equilibrium fill levels, or any mechanic where a pathogen
/// inoculates a host, quietly multiplies through every available
/// niche, and eventually tips the system into a full-blown
/// zymotic crisis that forces either crisis management or
/// complete suppression back to zero.
///
/// `inoculate(amount)` adds titer; fires `just_virulent` when
/// first reaching `max_titer`. No-op when disabled.
///
/// `suppress(amount)` reduces titer immediately; fires
/// `just_cleared` when reaching 0. No-op when disabled or already
/// cleared.
///
/// `tick(dt)` clears both flags, then increases titer by
/// `spread_rate * dt` (capped at `max_titer`). Fires `just_virulent`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_virulent()` returns `titer >= max_titer && enabled`.
///
/// `is_cleared()` returns `titer == 0.0` (not gated by `enabled`).
///
/// `titer_fraction()` returns `(titer / max_titer).clamp(0, 1)`.
///
/// `effective_contagion(scale)` returns `scale * titer_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — spreads at 2 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zymosis {
    pub titer: f32,
    pub max_titer: f32,
    pub spread_rate: f32,
    pub just_virulent: bool,
    pub just_cleared: bool,
    pub enabled: bool,
}

impl Zymosis {
    pub fn new(max_titer: f32, spread_rate: f32) -> Self {
        Self {
            titer: 0.0,
            max_titer: max_titer.max(0.1),
            spread_rate: spread_rate.max(0.0),
            just_virulent: false,
            just_cleared: false,
            enabled: true,
        }
    }

    /// Add titer; fires `just_virulent` when first reaching max.
    /// No-op when disabled.
    pub fn inoculate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.titer < self.max_titer;
        self.titer = (self.titer + amount).min(self.max_titer);
        if was_below && self.titer >= self.max_titer {
            self.just_virulent = true;
        }
    }

    /// Reduce titer; fires `just_cleared` when reaching 0.
    /// No-op when disabled or already cleared.
    pub fn suppress(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.titer <= 0.0 {
            return;
        }
        self.titer = (self.titer - amount).max(0.0);
        if self.titer <= 0.0 {
            self.just_cleared = true;
        }
    }

    /// Clear flags, then increase titer by `spread_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_virulent = false;
        self.just_cleared = false;
        if self.enabled && self.spread_rate > 0.0 && self.titer < self.max_titer {
            let was_below = self.titer < self.max_titer;
            self.titer = (self.titer + self.spread_rate * dt).min(self.max_titer);
            if was_below && self.titer >= self.max_titer {
                self.just_virulent = true;
            }
        }
    }

    /// `true` when titer is at maximum and component is enabled.
    pub fn is_virulent(&self) -> bool {
        self.titer >= self.max_titer && self.enabled
    }

    /// `true` when titer is 0 (not gated by `enabled`).
    pub fn is_cleared(&self) -> bool {
        self.titer == 0.0
    }

    /// Fraction of maximum titer [0.0, 1.0].
    pub fn titer_fraction(&self) -> f32 {
        (self.titer / self.max_titer).clamp(0.0, 1.0)
    }

    /// Returns `scale * titer_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_contagion(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.titer_fraction()
    }
}

impl Default for Zymosis {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zymosis {
        Zymosis::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_cleared() {
        let z = z();
        assert_eq!(z.titer, 0.0);
        assert!(z.is_cleared());
        assert!(!z.is_virulent());
    }

    #[test]
    fn new_clamps_max_titer() {
        let z = Zymosis::new(-5.0, 2.0);
        assert!((z.max_titer - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_spread_rate() {
        let z = Zymosis::new(100.0, -2.0);
        assert_eq!(z.spread_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zymosis::default();
        assert!((z.max_titer - 100.0).abs() < 1e-5);
        assert!((z.spread_rate - 2.0).abs() < 1e-5);
    }

    // --- inoculate ---

    #[test]
    fn inoculate_adds_titer() {
        let mut z = z();
        z.inoculate(40.0);
        assert!((z.titer - 40.0).abs() < 1e-3);
    }

    #[test]
    fn inoculate_clamps_at_max() {
        let mut z = z();
        z.inoculate(200.0);
        assert!((z.titer - 100.0).abs() < 1e-3);
    }

    #[test]
    fn inoculate_fires_just_virulent_at_max() {
        let mut z = z();
        z.inoculate(100.0);
        assert!(z.just_virulent);
        assert!(z.is_virulent());
    }

    #[test]
    fn inoculate_no_just_virulent_when_already_at_max() {
        let mut z = z();
        z.titer = 100.0;
        z.inoculate(10.0);
        assert!(!z.just_virulent);
    }

    #[test]
    fn inoculate_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.inoculate(50.0);
        assert_eq!(z.titer, 0.0);
    }

    #[test]
    fn inoculate_no_op_when_amount_zero() {
        let mut z = z();
        z.inoculate(0.0);
        assert_eq!(z.titer, 0.0);
    }

    // --- suppress ---

    #[test]
    fn suppress_reduces_titer() {
        let mut z = z();
        z.titer = 60.0;
        z.suppress(20.0);
        assert!((z.titer - 40.0).abs() < 1e-3);
    }

    #[test]
    fn suppress_clamps_at_zero() {
        let mut z = z();
        z.titer = 30.0;
        z.suppress(200.0);
        assert_eq!(z.titer, 0.0);
    }

    #[test]
    fn suppress_fires_just_cleared_at_zero() {
        let mut z = z();
        z.titer = 30.0;
        z.suppress(30.0);
        assert!(z.just_cleared);
    }

    #[test]
    fn suppress_no_op_when_already_cleared() {
        let mut z = z();
        z.suppress(10.0);
        assert!(!z.just_cleared);
    }

    #[test]
    fn suppress_no_op_when_disabled() {
        let mut z = z();
        z.titer = 50.0;
        z.enabled = false;
        z.suppress(50.0);
        assert!((z.titer - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_spreads_titer() {
        let mut z = z(); // rate=2
        z.tick(3.0); // 0 + 2*3 = 6
        assert!((z.titer - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_virulent_on_spread_to_max() {
        let mut z = Zymosis::new(100.0, 200.0);
        z.titer = 95.0;
        z.tick(1.0);
        assert!(z.just_virulent);
        assert!(z.is_virulent());
    }

    #[test]
    fn tick_no_spread_when_already_virulent() {
        let mut z = z();
        z.titer = 100.0;
        z.tick(1.0);
        assert!(!z.just_virulent);
    }

    #[test]
    fn tick_no_spread_when_rate_zero() {
        let mut z = Zymosis::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.titer, 0.0);
    }

    #[test]
    fn tick_no_spread_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.titer, 0.0);
    }

    #[test]
    fn tick_clears_just_virulent() {
        let mut z = Zymosis::new(100.0, 200.0);
        z.titer = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_virulent);
    }

    #[test]
    fn tick_clears_just_cleared() {
        let mut z = z();
        z.titer = 10.0;
        z.suppress(10.0);
        z.tick(0.016);
        assert!(!z.just_cleared);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2
        z.tick(5.0); // 2*5 = 10
        assert!((z.titer - 10.0).abs() < 1e-3);
    }

    // --- is_virulent / is_cleared ---

    #[test]
    fn is_virulent_false_when_disabled() {
        let mut z = z();
        z.titer = 100.0;
        z.enabled = false;
        assert!(!z.is_virulent());
    }

    #[test]
    fn is_cleared_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_cleared());
    }

    // --- titer_fraction / effective_contagion ---

    #[test]
    fn titer_fraction_zero_when_cleared() {
        assert_eq!(z().titer_fraction(), 0.0);
    }

    #[test]
    fn titer_fraction_half_at_midpoint() {
        let mut z = z();
        z.titer = 50.0;
        assert!((z.titer_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_contagion_zero_when_cleared() {
        assert_eq!(z().effective_contagion(100.0), 0.0);
    }

    #[test]
    fn effective_contagion_scales_with_titer() {
        let mut z = z();
        z.titer = 75.0;
        assert!((z.effective_contagion(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_contagion_zero_when_disabled() {
        let mut z = z();
        z.titer = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_contagion(100.0), 0.0);
    }
}
