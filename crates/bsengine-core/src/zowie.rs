use bevy_ecs::prelude::Component;

/// Spectacle-wow tracker. `spectacle` builds via `dazzle(amount)` and
/// intensifies passively at `wow_rate` per second in `tick(dt)` or
/// fades immediately via `fade(amount)`.
///
/// Models crowd-awe meters, combo-showmanship gauges, stunt-impact
/// accumulators, theatrical-presence fill levels, fireworks-charge bars,
/// audience-reaction trackers, performance-flair indicators, cinematic-
/// moment build-up gauges, or any mechanic where a spectacular display
/// accumulates audience excitement before it fades to normalcy.
///
/// `dazzle(amount)` adds spectacle; fires `just_dazzling` when first
/// reaching `max_spectacle`. No-op when disabled.
///
/// `fade(amount)` reduces spectacle immediately; fires `just_faded` when
/// reaching 0. No-op when disabled or already faded.
///
/// `tick(dt)` clears both flags, then increases spectacle by
/// `wow_rate * dt` (capped at `max_spectacle`). Fires `just_dazzling`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_dazzling()` returns `spectacle >= max_spectacle && enabled`.
///
/// `is_faded()` returns `spectacle == 0.0` (not gated by `enabled`).
///
/// `spectacle_fraction()` returns `(spectacle / max_spectacle).clamp(0, 1)`.
///
/// `effective_wow(scale)` returns `scale * spectacle_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 4.0)` — builds wow at 4 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zowie {
    pub spectacle: f32,
    pub max_spectacle: f32,
    pub wow_rate: f32,
    pub just_dazzling: bool,
    pub just_faded: bool,
    pub enabled: bool,
}

impl Zowie {
    pub fn new(max_spectacle: f32, wow_rate: f32) -> Self {
        Self {
            spectacle: 0.0,
            max_spectacle: max_spectacle.max(0.1),
            wow_rate: wow_rate.max(0.0),
            just_dazzling: false,
            just_faded: false,
            enabled: true,
        }
    }

    /// Add spectacle; fires `just_dazzling` when first reaching max.
    /// No-op when disabled.
    pub fn dazzle(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.spectacle < self.max_spectacle;
        self.spectacle = (self.spectacle + amount).min(self.max_spectacle);
        if was_below && self.spectacle >= self.max_spectacle {
            self.just_dazzling = true;
        }
    }

    /// Reduce spectacle; fires `just_faded` when reaching 0.
    /// No-op when disabled or already faded.
    pub fn fade(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.spectacle <= 0.0 {
            return;
        }
        self.spectacle = (self.spectacle - amount).max(0.0);
        if self.spectacle <= 0.0 {
            self.just_faded = true;
        }
    }

    /// Clear flags, then increase spectacle by `wow_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_dazzling = false;
        self.just_faded = false;
        if self.enabled && self.wow_rate > 0.0 && self.spectacle < self.max_spectacle {
            let was_below = self.spectacle < self.max_spectacle;
            self.spectacle = (self.spectacle + self.wow_rate * dt).min(self.max_spectacle);
            if was_below && self.spectacle >= self.max_spectacle {
                self.just_dazzling = true;
            }
        }
    }

    /// `true` when spectacle is at maximum and component is enabled.
    pub fn is_dazzling(&self) -> bool {
        self.spectacle >= self.max_spectacle && self.enabled
    }

    /// `true` when spectacle is 0 (not gated by `enabled`).
    pub fn is_faded(&self) -> bool {
        self.spectacle == 0.0
    }

    /// Fraction of maximum spectacle [0.0, 1.0].
    pub fn spectacle_fraction(&self) -> f32 {
        (self.spectacle / self.max_spectacle).clamp(0.0, 1.0)
    }

    /// Returns `scale * spectacle_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_wow(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.spectacle_fraction()
    }
}

impl Default for Zowie {
    fn default() -> Self {
        Self::new(100.0, 4.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zowie {
        Zowie::new(100.0, 4.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_faded() {
        let z = z();
        assert_eq!(z.spectacle, 0.0);
        assert!(z.is_faded());
        assert!(!z.is_dazzling());
    }

    #[test]
    fn new_clamps_max_spectacle() {
        let z = Zowie::new(-5.0, 4.0);
        assert!((z.max_spectacle - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_wow_rate() {
        let z = Zowie::new(100.0, -3.0);
        assert_eq!(z.wow_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zowie::default();
        assert!((z.max_spectacle - 100.0).abs() < 1e-5);
        assert!((z.wow_rate - 4.0).abs() < 1e-5);
    }

    // --- dazzle ---

    #[test]
    fn dazzle_adds_spectacle() {
        let mut z = z();
        z.dazzle(40.0);
        assert!((z.spectacle - 40.0).abs() < 1e-3);
    }

    #[test]
    fn dazzle_clamps_at_max() {
        let mut z = z();
        z.dazzle(200.0);
        assert!((z.spectacle - 100.0).abs() < 1e-3);
    }

    #[test]
    fn dazzle_fires_just_dazzling_at_max() {
        let mut z = z();
        z.dazzle(100.0);
        assert!(z.just_dazzling);
        assert!(z.is_dazzling());
    }

    #[test]
    fn dazzle_no_just_dazzling_when_already_at_max() {
        let mut z = z();
        z.spectacle = 100.0;
        z.dazzle(10.0);
        assert!(!z.just_dazzling);
    }

    #[test]
    fn dazzle_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.dazzle(50.0);
        assert_eq!(z.spectacle, 0.0);
    }

    #[test]
    fn dazzle_no_op_when_amount_zero() {
        let mut z = z();
        z.dazzle(0.0);
        assert_eq!(z.spectacle, 0.0);
    }

    // --- fade ---

    #[test]
    fn fade_reduces_spectacle() {
        let mut z = z();
        z.spectacle = 60.0;
        z.fade(20.0);
        assert!((z.spectacle - 40.0).abs() < 1e-3);
    }

    #[test]
    fn fade_clamps_at_zero() {
        let mut z = z();
        z.spectacle = 30.0;
        z.fade(200.0);
        assert_eq!(z.spectacle, 0.0);
    }

    #[test]
    fn fade_fires_just_faded_at_zero() {
        let mut z = z();
        z.spectacle = 30.0;
        z.fade(30.0);
        assert!(z.just_faded);
    }

    #[test]
    fn fade_no_op_when_already_faded() {
        let mut z = z();
        z.fade(10.0);
        assert!(!z.just_faded);
    }

    #[test]
    fn fade_no_op_when_disabled() {
        let mut z = z();
        z.spectacle = 50.0;
        z.enabled = false;
        z.fade(50.0);
        assert!((z.spectacle - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_spectacle() {
        let mut z = z(); // rate=4
        z.tick(1.0); // 0 + 4 = 4
        assert!((z.spectacle - 4.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_dazzling_on_wow_to_max() {
        let mut z = Zowie::new(100.0, 200.0);
        z.spectacle = 95.0;
        z.tick(1.0);
        assert!(z.just_dazzling);
        assert!(z.is_dazzling());
    }

    #[test]
    fn tick_no_wow_when_already_dazzling() {
        let mut z = z();
        z.spectacle = 100.0;
        z.tick(1.0);
        assert!(!z.just_dazzling);
    }

    #[test]
    fn tick_no_wow_when_rate_zero() {
        let mut z = Zowie::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.spectacle, 0.0);
    }

    #[test]
    fn tick_no_wow_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.spectacle, 0.0);
    }

    #[test]
    fn tick_clears_just_dazzling() {
        let mut z = Zowie::new(100.0, 200.0);
        z.spectacle = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_dazzling);
    }

    #[test]
    fn tick_clears_just_faded() {
        let mut z = z();
        z.spectacle = 10.0;
        z.fade(10.0);
        z.tick(0.016);
        assert!(!z.just_faded);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=4
        z.tick(3.0); // 4*3 = 12
        assert!((z.spectacle - 12.0).abs() < 1e-3);
    }

    // --- is_dazzling / is_faded ---

    #[test]
    fn is_dazzling_false_when_disabled() {
        let mut z = z();
        z.spectacle = 100.0;
        z.enabled = false;
        assert!(!z.is_dazzling());
    }

    #[test]
    fn is_faded_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_faded());
    }

    // --- spectacle_fraction / effective_wow ---

    #[test]
    fn spectacle_fraction_zero_when_faded() {
        assert_eq!(z().spectacle_fraction(), 0.0);
    }

    #[test]
    fn spectacle_fraction_half_at_midpoint() {
        let mut z = z();
        z.spectacle = 50.0;
        assert!((z.spectacle_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_wow_zero_when_faded() {
        assert_eq!(z().effective_wow(100.0), 0.0);
    }

    #[test]
    fn effective_wow_scales_with_spectacle() {
        let mut z = z();
        z.spectacle = 90.0;
        assert!((z.effective_wow(100.0) - 90.0).abs() < 1e-3);
    }

    #[test]
    fn effective_wow_zero_when_disabled() {
        let mut z = z();
        z.spectacle = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_wow(100.0), 0.0);
    }
}
