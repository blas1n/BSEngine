use bevy_ecs::prelude::Component;

/// Hidden-threat tension tracker. Models the rising unease when something
/// unseen but dangerous is nearby. `tension` builds through `sighting(amount)`
/// calls (proximity cues, environmental triggers) and fades passively at
/// `fade_rate` per second. `disperse(amount)` actively reduces tension.
///
/// Fires `just_manifested` when tension first reaches `max_tension`, and
/// `just_fled` when tension first returns to 0 from a non-zero value.
///
/// Models boss-approach telegraphing, lurking threat detection, cryptid
/// encounter systems, or any mechanic where off-screen threats build dread
/// through environmental cues before revealing themselves.
///
/// `sighting(amount)` adds to tension when below max. Fires `just_manifested`
/// on first reaching max. No-op when disabled or already at max.
///
/// `disperse(amount)` reduces tension when above 0. Fires `just_fled` when
/// tension first reaches 0. No-op when disabled.
///
/// `tick(dt)` clears `just_manifested` and `just_fled`. Then (when enabled
/// and `fade_rate > 0`) passively decays tension. Fires `just_fled` when
/// tension first reaches 0.
///
/// `is_manifested()` returns `tension >= max_tension && enabled`.
///
/// `is_fled()` returns `tension == 0.0` (not gated by `enabled`).
///
/// `tension_fraction()` returns `(tension / max_tension).clamp(0, 1)`.
///
/// `effective_dread(base)` returns `base * tension_fraction()` when enabled;
/// `0.0` when disabled.
///
/// Default: `new(100.0, 5.0)` — fades at 5/sec, starts at 0.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yeti {
    pub tension: f32,
    pub max_tension: f32,
    pub fade_rate: f32,
    pub just_manifested: bool,
    pub just_fled: bool,
    pub enabled: bool,
}

impl Yeti {
    pub fn new(max_tension: f32, fade_rate: f32) -> Self {
        Self {
            tension: 0.0,
            max_tension: max_tension.max(0.1),
            fade_rate: fade_rate.max(0.0),
            just_manifested: false,
            just_fled: false,
            enabled: true,
        }
    }

    /// Add a proximity cue; fires `just_manifested` on first reaching max.
    /// No-op when disabled or already at max.
    pub fn sighting(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.tension >= self.max_tension {
            return;
        }
        self.tension = (self.tension + amount).min(self.max_tension);
        if self.tension >= self.max_tension {
            self.just_manifested = true;
        }
    }

    /// Actively reduce tension; fires `just_fled` when reaching 0.
    /// No-op when disabled or tension already 0.
    pub fn disperse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.tension <= 0.0 {
            return;
        }
        self.tension = (self.tension - amount).max(0.0);
        if self.tension <= 0.0 {
            self.just_fled = true;
        }
    }

    /// Advance one frame: clear flags, then passively decay tension.
    /// Fires `just_fled` when tension first reaches 0 via decay.
    pub fn tick(&mut self, dt: f32) {
        self.just_manifested = false;
        self.just_fled = false;
        if self.enabled && self.fade_rate > 0.0 && self.tension > 0.0 {
            self.tension = (self.tension - self.fade_rate * dt).max(0.0);
            if self.tension <= 0.0 {
                self.just_fled = true;
            }
        }
    }

    /// `true` when tension is at maximum and component is enabled.
    pub fn is_manifested(&self) -> bool {
        self.tension >= self.max_tension && self.enabled
    }

    /// `true` when tension is 0 (not gated by `enabled`).
    pub fn is_fled(&self) -> bool {
        self.tension == 0.0
    }

    /// Fraction of maximum tension [0.0, 1.0].
    pub fn tension_fraction(&self) -> f32 {
        (self.tension / self.max_tension).clamp(0.0, 1.0)
    }

    /// Returns `base * tension_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_dread(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.tension_fraction()
    }
}

impl Default for Yeti {
    fn default() -> Self {
        Self::new(100.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yeti {
        Yeti::new(100.0, 10.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_calm() {
        let y = y();
        assert_eq!(y.tension, 0.0);
        assert!(y.is_fled());
        assert!(!y.is_manifested());
    }

    #[test]
    fn new_clamps_max_tension() {
        let y = Yeti::new(-5.0, 1.0);
        assert!((y.max_tension - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_fade_rate() {
        let y = Yeti::new(100.0, -5.0);
        assert_eq!(y.fade_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let y = Yeti::default();
        assert!((y.max_tension - 100.0).abs() < 1e-5);
        assert!((y.fade_rate - 5.0).abs() < 1e-5);
    }

    // --- sighting ---

    #[test]
    fn sighting_increases_tension() {
        let mut y = y();
        y.sighting(30.0);
        assert!((y.tension - 30.0).abs() < 1e-4);
    }

    #[test]
    fn sighting_clamps_at_max() {
        let mut y = y();
        y.sighting(200.0);
        assert!((y.tension - 100.0).abs() < 1e-5);
    }

    #[test]
    fn sighting_fires_just_manifested_at_max() {
        let mut y = y();
        y.sighting(100.0);
        assert!(y.just_manifested);
        assert!(y.is_manifested());
    }

    #[test]
    fn sighting_no_op_when_already_manifested() {
        let mut y = y();
        y.sighting(100.0);
        y.sighting(10.0); // already at max
        assert!(y.just_manifested); // still from first
    }

    #[test]
    fn sighting_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.sighting(50.0);
        assert_eq!(y.tension, 0.0);
    }

    #[test]
    fn sighting_no_op_for_zero_amount() {
        let mut y = y();
        y.sighting(0.0);
        assert_eq!(y.tension, 0.0);
    }

    // --- disperse ---

    #[test]
    fn disperse_reduces_tension() {
        let mut y = y();
        y.sighting(60.0);
        y.disperse(20.0);
        assert!((y.tension - 40.0).abs() < 1e-3);
    }

    #[test]
    fn disperse_clamps_at_zero() {
        let mut y = y();
        y.sighting(30.0);
        y.disperse(200.0);
        assert_eq!(y.tension, 0.0);
    }

    #[test]
    fn disperse_fires_just_fled_at_zero() {
        let mut y = y();
        y.sighting(30.0);
        y.disperse(30.0);
        assert!(y.just_fled);
        assert!(y.is_fled());
    }

    #[test]
    fn disperse_no_op_when_already_calm() {
        let mut y = y();
        y.disperse(10.0);
        assert!(!y.just_fled);
    }

    #[test]
    fn disperse_no_op_when_disabled() {
        let mut y = y();
        y.sighting(50.0);
        y.enabled = false;
        y.disperse(50.0);
        assert!((y.tension - 50.0).abs() < 1e-3);
    }

    // --- tick (passive fade) ---

    #[test]
    fn tick_fades_tension() {
        let mut y = y(); // fade_rate = 10
        y.sighting(50.0);
        y.tick(1.0); // 50 - 10 = 40
        assert!((y.tension - 40.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clamps_fade_at_zero() {
        let mut y = y();
        y.sighting(5.0);
        y.tick(10.0); // 5 - 100 → 0
        assert_eq!(y.tension, 0.0);
    }

    #[test]
    fn tick_fires_just_fled_on_fade_to_zero() {
        let mut y = y();
        y.sighting(5.0);
        y.tick(1.0);
        assert!(y.just_fled);
        assert!(y.is_fled());
    }

    #[test]
    fn tick_no_fled_when_already_calm() {
        let mut y = y();
        y.tick(1.0);
        assert!(!y.just_fled);
    }

    #[test]
    fn tick_clears_just_manifested() {
        let mut y = y();
        y.sighting(100.0);
        y.tick(0.001);
        assert!(!y.just_manifested);
    }

    #[test]
    fn tick_clears_just_fled() {
        let mut y = y();
        y.sighting(5.0);
        y.tick(1.0); // just_fled fires
        assert!(y.just_fled);
        y.tick(0.016); // cleared
        assert!(!y.just_fled);
    }

    #[test]
    fn tick_no_fade_when_rate_zero() {
        let mut y = Yeti::new(100.0, 0.0);
        y.sighting(50.0);
        y.tick(100.0);
        assert!((y.tension - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_fade_when_disabled() {
        let mut y = y();
        y.sighting(50.0);
        y.enabled = false;
        y.tick(1.0);
        assert!((y.tension - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut y = y();
        y.sighting(80.0);
        y.tick(0.5); // 80 - 10*0.5 = 75
        assert!((y.tension - 75.0).abs() < 1e-3);
    }

    // --- is_manifested / is_fled ---

    #[test]
    fn is_manifested_false_below_max() {
        let mut y = y();
        y.sighting(50.0);
        assert!(!y.is_manifested());
    }

    #[test]
    fn is_manifested_false_when_disabled() {
        let mut y = y();
        y.sighting(100.0);
        y.enabled = false;
        assert!(!y.is_manifested());
    }

    #[test]
    fn is_fled_true_at_zero() {
        assert!(y().is_fled());
    }

    #[test]
    fn is_fled_not_gated_by_enabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_fled());
    }

    // --- fractions / effective ---

    #[test]
    fn tension_fraction_zero_when_calm() {
        assert_eq!(y().tension_fraction(), 0.0);
    }

    #[test]
    fn tension_fraction_half_at_midpoint() {
        let mut y = y();
        y.sighting(50.0);
        assert!((y.tension_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn tension_fraction_one_at_max() {
        let mut y = y();
        y.sighting(100.0);
        assert!((y.tension_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_dread_zero_when_calm() {
        assert_eq!(y().effective_dread(100.0), 0.0);
    }

    #[test]
    fn effective_dread_scales_with_fraction() {
        let mut y = y();
        y.sighting(75.0);
        assert!((y.effective_dread(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_dread_zero_when_disabled() {
        let mut y = y();
        y.sighting(50.0);
        y.enabled = false;
        assert_eq!(y.effective_dread(100.0), 0.0);
    }
}
