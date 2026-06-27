use bevy_ecs::prelude::Component;

/// Fermentation-yield accumulation tracker. `brew` builds via
/// `mash(amount)` and ferments passively at `ferment_rate` per second
/// in `tick(dt)` or is drained immediately via `drain(amount)`.
///
/// Models ale-barrel fermentation progress bars, wort-to-wort
/// conversion fill levels, distillery-yield accumulation trackers,
/// mead-brewing saturation gauges, lager-lagering completeness
/// meters, spirits-aging fill levels, vinegar-culture build-up
/// indicators, kombucha-culture SCOBY-density bars, brew-kettle
/// temperature-window saturation trackers, or any mechanic where a
/// carefully balanced mash of grain, water, and yeast converts
/// sugars to alcohol in an orderly sequence of stages until every
/// last gram of fermentable substrate has been converted — only
/// for a sanitation failure or temperature spike to crash the
/// culture and send the barrel back to flat, spoiled wort.
///
/// `mash(amount)` adds brew; fires `just_fermented` when first
/// reaching `max_brew`. No-op when disabled.
///
/// `drain(amount)` reduces brew immediately; fires `just_flat`
/// when reaching 0. No-op when disabled or already flat.
///
/// `tick(dt)` clears both flags, then increases brew by
/// `ferment_rate * dt` (capped at `max_brew`). Fires
/// `just_fermented` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_fermented()` returns `brew >= max_brew && enabled`.
///
/// `is_flat()` returns `brew == 0.0` (not gated by `enabled`).
///
/// `brew_fraction()` returns `(brew / max_brew).clamp(0, 1)`.
///
/// `effective_potency(scale)` returns `scale * brew_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.0)` — ferments at 1 unit/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zymurgy {
    pub brew: f32,
    pub max_brew: f32,
    pub ferment_rate: f32,
    pub just_fermented: bool,
    pub just_flat: bool,
    pub enabled: bool,
}

impl Zymurgy {
    pub fn new(max_brew: f32, ferment_rate: f32) -> Self {
        Self {
            brew: 0.0,
            max_brew: max_brew.max(0.1),
            ferment_rate: ferment_rate.max(0.0),
            just_fermented: false,
            just_flat: false,
            enabled: true,
        }
    }

    /// Add brew; fires `just_fermented` when first reaching max.
    /// No-op when disabled.
    pub fn mash(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.brew < self.max_brew;
        self.brew = (self.brew + amount).min(self.max_brew);
        if was_below && self.brew >= self.max_brew {
            self.just_fermented = true;
        }
    }

    /// Reduce brew; fires `just_flat` when reaching 0.
    /// No-op when disabled or already flat.
    pub fn drain(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.brew <= 0.0 {
            return;
        }
        self.brew = (self.brew - amount).max(0.0);
        if self.brew <= 0.0 {
            self.just_flat = true;
        }
    }

    /// Clear flags, then increase brew by `ferment_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_fermented = false;
        self.just_flat = false;
        if self.enabled && self.ferment_rate > 0.0 && self.brew < self.max_brew {
            let was_below = self.brew < self.max_brew;
            self.brew = (self.brew + self.ferment_rate * dt).min(self.max_brew);
            if was_below && self.brew >= self.max_brew {
                self.just_fermented = true;
            }
        }
    }

    /// `true` when brew is at maximum and component is enabled.
    pub fn is_fermented(&self) -> bool {
        self.brew >= self.max_brew && self.enabled
    }

    /// `true` when brew is 0 (not gated by `enabled`).
    pub fn is_flat(&self) -> bool {
        self.brew == 0.0
    }

    /// Fraction of maximum brew [0.0, 1.0].
    pub fn brew_fraction(&self) -> f32 {
        (self.brew / self.max_brew).clamp(0.0, 1.0)
    }

    /// Returns `scale * brew_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_potency(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.brew_fraction()
    }
}

impl Default for Zymurgy {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zymurgy {
        Zymurgy::new(100.0, 1.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_flat() {
        let z = z();
        assert_eq!(z.brew, 0.0);
        assert!(z.is_flat());
        assert!(!z.is_fermented());
    }

    #[test]
    fn new_clamps_max_brew() {
        let z = Zymurgy::new(-5.0, 1.0);
        assert!((z.max_brew - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_ferment_rate() {
        let z = Zymurgy::new(100.0, -1.0);
        assert_eq!(z.ferment_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zymurgy::default();
        assert!((z.max_brew - 100.0).abs() < 1e-5);
        assert!((z.ferment_rate - 1.0).abs() < 1e-5);
    }

    // --- mash ---

    #[test]
    fn mash_adds_brew() {
        let mut z = z();
        z.mash(40.0);
        assert!((z.brew - 40.0).abs() < 1e-3);
    }

    #[test]
    fn mash_clamps_at_max() {
        let mut z = z();
        z.mash(200.0);
        assert!((z.brew - 100.0).abs() < 1e-3);
    }

    #[test]
    fn mash_fires_just_fermented_at_max() {
        let mut z = z();
        z.mash(100.0);
        assert!(z.just_fermented);
        assert!(z.is_fermented());
    }

    #[test]
    fn mash_no_just_fermented_when_already_at_max() {
        let mut z = z();
        z.brew = 100.0;
        z.mash(10.0);
        assert!(!z.just_fermented);
    }

    #[test]
    fn mash_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.mash(50.0);
        assert_eq!(z.brew, 0.0);
    }

    #[test]
    fn mash_no_op_when_amount_zero() {
        let mut z = z();
        z.mash(0.0);
        assert_eq!(z.brew, 0.0);
    }

    // --- drain ---

    #[test]
    fn drain_reduces_brew() {
        let mut z = z();
        z.brew = 60.0;
        z.drain(20.0);
        assert!((z.brew - 40.0).abs() < 1e-3);
    }

    #[test]
    fn drain_clamps_at_zero() {
        let mut z = z();
        z.brew = 30.0;
        z.drain(200.0);
        assert_eq!(z.brew, 0.0);
    }

    #[test]
    fn drain_fires_just_flat_at_zero() {
        let mut z = z();
        z.brew = 30.0;
        z.drain(30.0);
        assert!(z.just_flat);
    }

    #[test]
    fn drain_no_op_when_already_flat() {
        let mut z = z();
        z.drain(10.0);
        assert!(!z.just_flat);
    }

    #[test]
    fn drain_no_op_when_disabled() {
        let mut z = z();
        z.brew = 50.0;
        z.enabled = false;
        z.drain(50.0);
        assert!((z.brew - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_ferments_brew() {
        let mut z = z(); // rate=1
        z.tick(5.0); // 0 + 1*5 = 5
        assert!((z.brew - 5.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_fermented_on_ferment_to_max() {
        let mut z = Zymurgy::new(100.0, 200.0);
        z.brew = 95.0;
        z.tick(1.0);
        assert!(z.just_fermented);
        assert!(z.is_fermented());
    }

    #[test]
    fn tick_no_ferment_when_already_fermented() {
        let mut z = z();
        z.brew = 100.0;
        z.tick(1.0);
        assert!(!z.just_fermented);
    }

    #[test]
    fn tick_no_ferment_when_rate_zero() {
        let mut z = Zymurgy::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.brew, 0.0);
    }

    #[test]
    fn tick_no_ferment_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.brew, 0.0);
    }

    #[test]
    fn tick_clears_just_fermented() {
        let mut z = Zymurgy::new(100.0, 200.0);
        z.brew = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_fermented);
    }

    #[test]
    fn tick_clears_just_flat() {
        let mut z = z();
        z.brew = 10.0;
        z.drain(10.0);
        z.tick(0.016);
        assert!(!z.just_flat);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1
        z.tick(8.0); // 1*8 = 8
        assert!((z.brew - 8.0).abs() < 1e-3);
    }

    // --- is_fermented / is_flat ---

    #[test]
    fn is_fermented_false_when_disabled() {
        let mut z = z();
        z.brew = 100.0;
        z.enabled = false;
        assert!(!z.is_fermented());
    }

    #[test]
    fn is_flat_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_flat());
    }

    // --- brew_fraction / effective_potency ---

    #[test]
    fn brew_fraction_zero_when_flat() {
        assert_eq!(z().brew_fraction(), 0.0);
    }

    #[test]
    fn brew_fraction_half_at_midpoint() {
        let mut z = z();
        z.brew = 50.0;
        assert!((z.brew_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_potency_zero_when_flat() {
        assert_eq!(z().effective_potency(100.0), 0.0);
    }

    #[test]
    fn effective_potency_scales_with_brew() {
        let mut z = z();
        z.brew = 75.0;
        assert!((z.effective_potency(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_potency_zero_when_disabled() {
        let mut z = z();
        z.brew = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_potency(100.0), 0.0);
    }
}
