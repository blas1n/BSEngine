use bevy_ecs::prelude::Component;

/// Afro-Creole rhythm-energy tracker. `groove` builds via
/// `play(amount)` and intensifies passively at `rhythm_rate` per
/// second in `tick(dt)` or fades immediately via `fade(amount)`.
///
/// Models Louisiana-Cajun dance-floor energy fill levels, zydeco-
/// accordion groove saturation bars, Creole rhythm-intensity
/// accumulators, washboard-scrub momentum gauges, dance-hall energy
/// build-up indicators, call-and-response saturation trackers,
/// blues-infused groove progression meters, second-line parade
/// energy fill levels, accordion-driven beat-intensity bars, or any
/// mechanic where a fais-do-do slowly builds from a shy two-step
/// to a floor-shaking communal stomp where everyone — grandmothers
/// included — finds themselves unable to stand still until the
/// accordionist finally sets down the instrument and the hall goes
/// quiet all at once.
///
/// `play(amount)` adds groove; fires `just_rocking` when first
/// reaching `max_groove`. No-op when disabled.
///
/// `fade(amount)` reduces groove immediately; fires `just_silent`
/// when reaching 0. No-op when disabled or already silent.
///
/// `tick(dt)` clears both flags, then increases groove by
/// `rhythm_rate * dt` (capped at `max_groove`). Fires `just_rocking`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_rocking()` returns `groove >= max_groove && enabled`.
///
/// `is_silent()` returns `groove == 0.0` (not gated by `enabled`).
///
/// `groove_fraction()` returns `(groove / max_groove).clamp(0, 1)`.
///
/// `effective_energy(scale)` returns `scale * groove_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — builds rhythm at 2 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zydeco {
    pub groove: f32,
    pub max_groove: f32,
    pub rhythm_rate: f32,
    pub just_rocking: bool,
    pub just_silent: bool,
    pub enabled: bool,
}

impl Zydeco {
    pub fn new(max_groove: f32, rhythm_rate: f32) -> Self {
        Self {
            groove: 0.0,
            max_groove: max_groove.max(0.1),
            rhythm_rate: rhythm_rate.max(0.0),
            just_rocking: false,
            just_silent: false,
            enabled: true,
        }
    }

    /// Add groove; fires `just_rocking` when first reaching max.
    /// No-op when disabled.
    pub fn play(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.groove < self.max_groove;
        self.groove = (self.groove + amount).min(self.max_groove);
        if was_below && self.groove >= self.max_groove {
            self.just_rocking = true;
        }
    }

    /// Reduce groove; fires `just_silent` when reaching 0.
    /// No-op when disabled or already silent.
    pub fn fade(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.groove <= 0.0 {
            return;
        }
        self.groove = (self.groove - amount).max(0.0);
        if self.groove <= 0.0 {
            self.just_silent = true;
        }
    }

    /// Clear flags, then increase groove by `rhythm_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_rocking = false;
        self.just_silent = false;
        if self.enabled && self.rhythm_rate > 0.0 && self.groove < self.max_groove {
            let was_below = self.groove < self.max_groove;
            self.groove = (self.groove + self.rhythm_rate * dt).min(self.max_groove);
            if was_below && self.groove >= self.max_groove {
                self.just_rocking = true;
            }
        }
    }

    /// `true` when groove is at maximum and component is enabled.
    pub fn is_rocking(&self) -> bool {
        self.groove >= self.max_groove && self.enabled
    }

    /// `true` when groove is 0 (not gated by `enabled`).
    pub fn is_silent(&self) -> bool {
        self.groove == 0.0
    }

    /// Fraction of maximum groove [0.0, 1.0].
    pub fn groove_fraction(&self) -> f32 {
        (self.groove / self.max_groove).clamp(0.0, 1.0)
    }

    /// Returns `scale * groove_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_energy(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.groove_fraction()
    }
}

impl Default for Zydeco {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zydeco {
        Zydeco::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_silent() {
        let z = z();
        assert_eq!(z.groove, 0.0);
        assert!(z.is_silent());
        assert!(!z.is_rocking());
    }

    #[test]
    fn new_clamps_max_groove() {
        let z = Zydeco::new(-5.0, 2.0);
        assert!((z.max_groove - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_rhythm_rate() {
        let z = Zydeco::new(100.0, -2.0);
        assert_eq!(z.rhythm_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zydeco::default();
        assert!((z.max_groove - 100.0).abs() < 1e-5);
        assert!((z.rhythm_rate - 2.0).abs() < 1e-5);
    }

    // --- play ---

    #[test]
    fn play_adds_groove() {
        let mut z = z();
        z.play(40.0);
        assert!((z.groove - 40.0).abs() < 1e-3);
    }

    #[test]
    fn play_clamps_at_max() {
        let mut z = z();
        z.play(200.0);
        assert!((z.groove - 100.0).abs() < 1e-3);
    }

    #[test]
    fn play_fires_just_rocking_at_max() {
        let mut z = z();
        z.play(100.0);
        assert!(z.just_rocking);
        assert!(z.is_rocking());
    }

    #[test]
    fn play_no_just_rocking_when_already_at_max() {
        let mut z = z();
        z.groove = 100.0;
        z.play(10.0);
        assert!(!z.just_rocking);
    }

    #[test]
    fn play_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.play(50.0);
        assert_eq!(z.groove, 0.0);
    }

    #[test]
    fn play_no_op_when_amount_zero() {
        let mut z = z();
        z.play(0.0);
        assert_eq!(z.groove, 0.0);
    }

    // --- fade ---

    #[test]
    fn fade_reduces_groove() {
        let mut z = z();
        z.groove = 60.0;
        z.fade(20.0);
        assert!((z.groove - 40.0).abs() < 1e-3);
    }

    #[test]
    fn fade_clamps_at_zero() {
        let mut z = z();
        z.groove = 30.0;
        z.fade(200.0);
        assert_eq!(z.groove, 0.0);
    }

    #[test]
    fn fade_fires_just_silent_at_zero() {
        let mut z = z();
        z.groove = 30.0;
        z.fade(30.0);
        assert!(z.just_silent);
    }

    #[test]
    fn fade_no_op_when_already_silent() {
        let mut z = z();
        z.fade(10.0);
        assert!(!z.just_silent);
    }

    #[test]
    fn fade_no_op_when_disabled() {
        let mut z = z();
        z.groove = 50.0;
        z.enabled = false;
        z.fade(50.0);
        assert!((z.groove - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_groove() {
        let mut z = z(); // rate=2
        z.tick(3.0); // 0 + 2*3 = 6
        assert!((z.groove - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_rocking_on_build_to_max() {
        let mut z = Zydeco::new(100.0, 200.0);
        z.groove = 95.0;
        z.tick(1.0);
        assert!(z.just_rocking);
        assert!(z.is_rocking());
    }

    #[test]
    fn tick_no_build_when_already_rocking() {
        let mut z = z();
        z.groove = 100.0;
        z.tick(1.0);
        assert!(!z.just_rocking);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut z = Zydeco::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.groove, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.groove, 0.0);
    }

    #[test]
    fn tick_clears_just_rocking() {
        let mut z = Zydeco::new(100.0, 200.0);
        z.groove = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_rocking);
    }

    #[test]
    fn tick_clears_just_silent() {
        let mut z = z();
        z.groove = 10.0;
        z.fade(10.0);
        z.tick(0.016);
        assert!(!z.just_silent);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2
        z.tick(5.0); // 2*5 = 10
        assert!((z.groove - 10.0).abs() < 1e-3);
    }

    // --- is_rocking / is_silent ---

    #[test]
    fn is_rocking_false_when_disabled() {
        let mut z = z();
        z.groove = 100.0;
        z.enabled = false;
        assert!(!z.is_rocking());
    }

    #[test]
    fn is_silent_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_silent());
    }

    // --- groove_fraction / effective_energy ---

    #[test]
    fn groove_fraction_zero_when_silent() {
        assert_eq!(z().groove_fraction(), 0.0);
    }

    #[test]
    fn groove_fraction_half_at_midpoint() {
        let mut z = z();
        z.groove = 50.0;
        assert!((z.groove_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_energy_zero_when_silent() {
        assert_eq!(z().effective_energy(100.0), 0.0);
    }

    #[test]
    fn effective_energy_scales_with_groove() {
        let mut z = z();
        z.groove = 75.0;
        assert!((z.effective_energy(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_energy_zero_when_disabled() {
        let mut z = z();
        z.groove = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_energy(100.0), 0.0);
    }
}
