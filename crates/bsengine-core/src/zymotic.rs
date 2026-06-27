use bevy_ecs::prelude::Component;

/// Epidemic-spread intensity tracker. `contagion` builds via `expose(amount)`
/// and spreads passively at `spread_rate` per second in `tick(dt)` or
/// is suppressed immediately via `contain(amount)`.
///
/// Models disease-outbreak saturation bars, epidemic-progression fill
/// levels, contagion-pressure intensity gauges, pathogen-load
/// accumulation trackers, population-infection spread meters,
/// quarantine-breach risk indicators, biological-hazard build-up
/// bars, viral-propagation intensity fill levels, fermentation-
/// spoilage progress trackers, or any mechanic where a microscopic
/// agent multiplies through a population until every susceptible
/// host is compromised — only for a vaccine drive or antimicrobial
/// intervention to suppress the outbreak back to baseline.
///
/// `expose(amount)` adds contagion; fires `just_epidemic` when first
/// reaching `max_contagion`. No-op when disabled.
///
/// `contain(amount)` reduces contagion immediately; fires `just_clear`
/// when reaching 0. No-op when disabled or already clear.
///
/// `tick(dt)` clears both flags, then increases contagion by
/// `spread_rate * dt` (capped at `max_contagion`). Fires
/// `just_epidemic` when first reaching max. No-op when disabled or
/// rate is 0.
///
/// `is_epidemic()` returns `contagion >= max_contagion && enabled`.
///
/// `is_clear()` returns `contagion == 0.0` (not gated by `enabled`).
///
/// `contagion_fraction()` returns `(contagion / max_contagion).clamp(0, 1)`.
///
/// `effective_virulence(scale)` returns `scale * contagion_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — spreads at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zymotic {
    pub contagion: f32,
    pub max_contagion: f32,
    pub spread_rate: f32,
    pub just_epidemic: bool,
    pub just_clear: bool,
    pub enabled: bool,
}

impl Zymotic {
    pub fn new(max_contagion: f32, spread_rate: f32) -> Self {
        Self {
            contagion: 0.0,
            max_contagion: max_contagion.max(0.1),
            spread_rate: spread_rate.max(0.0),
            just_epidemic: false,
            just_clear: false,
            enabled: true,
        }
    }

    /// Add contagion; fires `just_epidemic` when first reaching max.
    /// No-op when disabled.
    pub fn expose(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.contagion < self.max_contagion;
        self.contagion = (self.contagion + amount).min(self.max_contagion);
        if was_below && self.contagion >= self.max_contagion {
            self.just_epidemic = true;
        }
    }

    /// Reduce contagion; fires `just_clear` when reaching 0.
    /// No-op when disabled or already clear.
    pub fn contain(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.contagion <= 0.0 {
            return;
        }
        self.contagion = (self.contagion - amount).max(0.0);
        if self.contagion <= 0.0 {
            self.just_clear = true;
        }
    }

    /// Clear flags, then increase contagion by `spread_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_epidemic = false;
        self.just_clear = false;
        if self.enabled && self.spread_rate > 0.0 && self.contagion < self.max_contagion {
            let was_below = self.contagion < self.max_contagion;
            self.contagion = (self.contagion + self.spread_rate * dt).min(self.max_contagion);
            if was_below && self.contagion >= self.max_contagion {
                self.just_epidemic = true;
            }
        }
    }

    /// `true` when contagion is at maximum and component is enabled.
    pub fn is_epidemic(&self) -> bool {
        self.contagion >= self.max_contagion && self.enabled
    }

    /// `true` when contagion is 0 (not gated by `enabled`).
    pub fn is_clear(&self) -> bool {
        self.contagion == 0.0
    }

    /// Fraction of maximum contagion [0.0, 1.0].
    pub fn contagion_fraction(&self) -> f32 {
        (self.contagion / self.max_contagion).clamp(0.0, 1.0)
    }

    /// Returns `scale * contagion_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_virulence(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.contagion_fraction()
    }
}

impl Default for Zymotic {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zymotic {
        Zymotic::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_clear() {
        let z = z();
        assert_eq!(z.contagion, 0.0);
        assert!(z.is_clear());
        assert!(!z.is_epidemic());
    }

    #[test]
    fn new_clamps_max_contagion() {
        let z = Zymotic::new(-5.0, 1.5);
        assert!((z.max_contagion - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_spread_rate() {
        let z = Zymotic::new(100.0, -1.5);
        assert_eq!(z.spread_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zymotic::default();
        assert!((z.max_contagion - 100.0).abs() < 1e-5);
        assert!((z.spread_rate - 1.5).abs() < 1e-5);
    }

    // --- expose ---

    #[test]
    fn expose_adds_contagion() {
        let mut z = z();
        z.expose(40.0);
        assert!((z.contagion - 40.0).abs() < 1e-3);
    }

    #[test]
    fn expose_clamps_at_max() {
        let mut z = z();
        z.expose(200.0);
        assert!((z.contagion - 100.0).abs() < 1e-3);
    }

    #[test]
    fn expose_fires_just_epidemic_at_max() {
        let mut z = z();
        z.expose(100.0);
        assert!(z.just_epidemic);
        assert!(z.is_epidemic());
    }

    #[test]
    fn expose_no_just_epidemic_when_already_at_max() {
        let mut z = z();
        z.contagion = 100.0;
        z.expose(10.0);
        assert!(!z.just_epidemic);
    }

    #[test]
    fn expose_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.expose(50.0);
        assert_eq!(z.contagion, 0.0);
    }

    #[test]
    fn expose_no_op_when_amount_zero() {
        let mut z = z();
        z.expose(0.0);
        assert_eq!(z.contagion, 0.0);
    }

    // --- contain ---

    #[test]
    fn contain_reduces_contagion() {
        let mut z = z();
        z.contagion = 60.0;
        z.contain(20.0);
        assert!((z.contagion - 40.0).abs() < 1e-3);
    }

    #[test]
    fn contain_clamps_at_zero() {
        let mut z = z();
        z.contagion = 30.0;
        z.contain(200.0);
        assert_eq!(z.contagion, 0.0);
    }

    #[test]
    fn contain_fires_just_clear_at_zero() {
        let mut z = z();
        z.contagion = 30.0;
        z.contain(30.0);
        assert!(z.just_clear);
    }

    #[test]
    fn contain_no_op_when_already_clear() {
        let mut z = z();
        z.contain(10.0);
        assert!(!z.just_clear);
    }

    #[test]
    fn contain_no_op_when_disabled() {
        let mut z = z();
        z.contagion = 50.0;
        z.enabled = false;
        z.contain(50.0);
        assert!((z.contagion - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_spreads_contagion() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.contagion - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_epidemic_on_spread_to_max() {
        let mut z = Zymotic::new(100.0, 200.0);
        z.contagion = 95.0;
        z.tick(1.0);
        assert!(z.just_epidemic);
        assert!(z.is_epidemic());
    }

    #[test]
    fn tick_no_spread_when_already_epidemic() {
        let mut z = z();
        z.contagion = 100.0;
        z.tick(1.0);
        assert!(!z.just_epidemic);
    }

    #[test]
    fn tick_no_spread_when_rate_zero() {
        let mut z = Zymotic::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.contagion, 0.0);
    }

    #[test]
    fn tick_no_spread_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.contagion, 0.0);
    }

    #[test]
    fn tick_clears_just_epidemic() {
        let mut z = Zymotic::new(100.0, 200.0);
        z.contagion = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_epidemic);
    }

    #[test]
    fn tick_clears_just_clear() {
        let mut z = z();
        z.contagion = 10.0;
        z.contain(10.0);
        z.tick(0.016);
        assert!(!z.just_clear);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.contagion - 9.0).abs() < 1e-3);
    }

    // --- is_epidemic / is_clear ---

    #[test]
    fn is_epidemic_false_when_disabled() {
        let mut z = z();
        z.contagion = 100.0;
        z.enabled = false;
        assert!(!z.is_epidemic());
    }

    #[test]
    fn is_clear_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_clear());
    }

    // --- contagion_fraction / effective_virulence ---

    #[test]
    fn contagion_fraction_zero_when_clear() {
        assert_eq!(z().contagion_fraction(), 0.0);
    }

    #[test]
    fn contagion_fraction_half_at_midpoint() {
        let mut z = z();
        z.contagion = 50.0;
        assert!((z.contagion_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_virulence_zero_when_clear() {
        assert_eq!(z().effective_virulence(100.0), 0.0);
    }

    #[test]
    fn effective_virulence_scales_with_contagion() {
        let mut z = z();
        z.contagion = 75.0;
        assert!((z.effective_virulence(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_virulence_zero_when_disabled() {
        let mut z = z();
        z.contagion = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_virulence(100.0), 0.0);
    }
}
