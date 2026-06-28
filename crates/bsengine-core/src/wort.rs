use bevy_ecs::prelude::Component;

/// Fermentation-brew accumulation tracker named after wort, the
/// noun meaning the liquid extracted from the mashing process
/// during the brewing of beer or whisky, prior to fermentation;
/// also a suffix or word meaning a plant or herb of a specified
/// kind — from the Old English wyrt (plant, herb, root, spice),
/// from the Proto-Germanic wurtiz (root, plant), from the Proto-
/// Indo-European root wrād- (root, branch, twig). The root
/// wrād- gave Latin radix (root), rādīx (radish), Sanskrit
/// vrā́dhati (grows, prospers), and through them the English
/// words radical, radish, and (via Old French) licorice and
/// carrot. In its brewing sense, wort is the sweet, unfermented
/// liquid produced by steeping malted grain in hot water — it
/// is what the yeast eats to produce alcohol, the raw material
/// of fermentation, the stage in which complex sugars extracted
/// from grain are held in suspension, awaiting transformation.
/// The wort must be cooled, transferred to a fermentation vessel,
/// and inoculated with yeast before it can become beer; the
/// time it spends as wort is the window of maximum biological
/// potential, the moment before the process that makes it what
/// it will eventually be has begun. In its botanical sense,
/// wort survives in compound names: liverwort (a plant once
/// thought to be good for the liver), spiderwort (a plant
/// the texture of whose sap resembles a spider's web), mugwort
/// (a plant once used to flavour ale before hops became
/// standard). In game mechanics, a wort mechanic models the
/// slow accumulation of raw, unprocessed potential — the fill
/// of a brewing vat, the build of fermentation readiness,
/// the accumulation of ingredients awaiting transformation.
/// `brew` builds via `mash(amount)` and accumulates passively
/// at `ferment_rate` per second in `tick(dt)` or is consumed
/// via `pour(amount)`.
///
/// Models fermentation-brew fill levels, alchemy-saturation
/// bars, mash-accumulation trackers, brew-readiness gauges,
/// potion-craft fill levels, herbal-saturation indicators,
/// tincture-accumulation bars, distillation meters, craft-
/// completion fill levels, or any mechanic where a character,
/// station, or entity slowly accumulates the raw material,
/// steeping liquid, or unfermented potential required to
/// produce a final product — each ingredient added, each
/// degree of temperature maintained, each hour of steeping
/// time adding a fraction of readiness until the threshold
/// is crossed and the brew is ready to become what it was
/// always meant to be.
///
/// `mash(amount)` adds brew; fires `just_brewed` when first
/// reaching `max_brew`. No-op when disabled.
///
/// `pour(amount)` reduces brew immediately; fires `just_dry`
/// when reaching 0. No-op when disabled or already dry.
///
/// `tick(dt)` clears both flags, then increases brew by
/// `ferment_rate * dt` (capped at `max_brew`). Fires
/// `just_brewed` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_brewed()` returns `brew >= max_brew && enabled`.
///
/// `is_dry()` returns `brew == 0.0` (not gated by `enabled`).
///
/// `brew_fraction()` returns `(brew / max_brew).clamp(0, 1)`.
///
/// `effective_potency(scale)` returns `scale * brew_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — ferments at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wort {
    pub brew: f32,
    pub max_brew: f32,
    pub ferment_rate: f32,
    pub just_brewed: bool,
    pub just_dry: bool,
    pub enabled: bool,
}

impl Wort {
    pub fn new(max_brew: f32, ferment_rate: f32) -> Self {
        Self {
            brew: 0.0,
            max_brew: max_brew.max(0.1),
            ferment_rate: ferment_rate.max(0.0),
            just_brewed: false,
            just_dry: false,
            enabled: true,
        }
    }

    /// Add brew; fires `just_brewed` when first reaching max.
    /// No-op when disabled.
    pub fn mash(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.brew < self.max_brew;
        self.brew = (self.brew + amount).min(self.max_brew);
        if was_below && self.brew >= self.max_brew {
            self.just_brewed = true;
        }
    }

    /// Reduce brew; fires `just_dry` when reaching 0.
    /// No-op when disabled or already dry.
    pub fn pour(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.brew <= 0.0 {
            return;
        }
        self.brew = (self.brew - amount).max(0.0);
        if self.brew <= 0.0 {
            self.just_dry = true;
        }
    }

    /// Clear flags, then increase brew by `ferment_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_brewed = false;
        self.just_dry = false;
        if self.enabled && self.ferment_rate > 0.0 && self.brew < self.max_brew {
            let was_below = self.brew < self.max_brew;
            self.brew = (self.brew + self.ferment_rate * dt).min(self.max_brew);
            if was_below && self.brew >= self.max_brew {
                self.just_brewed = true;
            }
        }
    }

    /// `true` when brew is at maximum and component is enabled.
    pub fn is_brewed(&self) -> bool {
        self.brew >= self.max_brew && self.enabled
    }

    /// `true` when brew is 0 (not gated by `enabled`).
    pub fn is_dry(&self) -> bool {
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

impl Default for Wort {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wort {
        Wort::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_dry() {
        let w = w();
        assert_eq!(w.brew, 0.0);
        assert!(w.is_dry());
        assert!(!w.is_brewed());
    }

    #[test]
    fn new_clamps_max_brew() {
        let w = Wort::new(-5.0, 1.5);
        assert!((w.max_brew - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_ferment_rate() {
        let w = Wort::new(100.0, -1.5);
        assert_eq!(w.ferment_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Wort::default();
        assert!((w.max_brew - 100.0).abs() < 1e-5);
        assert!((w.ferment_rate - 1.5).abs() < 1e-5);
    }

    // --- mash ---

    #[test]
    fn mash_adds_brew() {
        let mut w = w();
        w.mash(40.0);
        assert!((w.brew - 40.0).abs() < 1e-3);
    }

    #[test]
    fn mash_clamps_at_max() {
        let mut w = w();
        w.mash(200.0);
        assert!((w.brew - 100.0).abs() < 1e-3);
    }

    #[test]
    fn mash_fires_just_brewed_at_max() {
        let mut w = w();
        w.mash(100.0);
        assert!(w.just_brewed);
        assert!(w.is_brewed());
    }

    #[test]
    fn mash_no_just_brewed_when_already_at_max() {
        let mut w = w();
        w.brew = 100.0;
        w.mash(10.0);
        assert!(!w.just_brewed);
    }

    #[test]
    fn mash_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.mash(50.0);
        assert_eq!(w.brew, 0.0);
    }

    #[test]
    fn mash_no_op_when_amount_zero() {
        let mut w = w();
        w.mash(0.0);
        assert_eq!(w.brew, 0.0);
    }

    // --- pour ---

    #[test]
    fn pour_reduces_brew() {
        let mut w = w();
        w.brew = 60.0;
        w.pour(20.0);
        assert!((w.brew - 40.0).abs() < 1e-3);
    }

    #[test]
    fn pour_clamps_at_zero() {
        let mut w = w();
        w.brew = 30.0;
        w.pour(200.0);
        assert_eq!(w.brew, 0.0);
    }

    #[test]
    fn pour_fires_just_dry_at_zero() {
        let mut w = w();
        w.brew = 30.0;
        w.pour(30.0);
        assert!(w.just_dry);
    }

    #[test]
    fn pour_no_op_when_already_dry() {
        let mut w = w();
        w.pour(10.0);
        assert!(!w.just_dry);
    }

    #[test]
    fn pour_no_op_when_disabled() {
        let mut w = w();
        w.brew = 50.0;
        w.enabled = false;
        w.pour(50.0);
        assert!((w.brew - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_brew() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.brew - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_brewed_on_brew_to_max() {
        let mut w = Wort::new(100.0, 200.0);
        w.brew = 95.0;
        w.tick(1.0);
        assert!(w.just_brewed);
        assert!(w.is_brewed());
    }

    #[test]
    fn tick_no_build_when_already_brewed() {
        let mut w = w();
        w.brew = 100.0;
        w.tick(1.0);
        assert!(!w.just_brewed);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Wort::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.brew, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.brew, 0.0);
    }

    #[test]
    fn tick_clears_just_brewed() {
        let mut w = Wort::new(100.0, 200.0);
        w.brew = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_brewed);
    }

    #[test]
    fn tick_clears_just_dry() {
        let mut w = w();
        w.brew = 10.0;
        w.pour(10.0);
        w.tick(0.016);
        assert!(!w.just_dry);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.brew - 9.0).abs() < 1e-3);
    }

    // --- is_brewed / is_dry ---

    #[test]
    fn is_brewed_false_when_disabled() {
        let mut w = w();
        w.brew = 100.0;
        w.enabled = false;
        assert!(!w.is_brewed());
    }

    #[test]
    fn is_dry_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_dry());
    }

    // --- brew_fraction / effective_potency ---

    #[test]
    fn brew_fraction_zero_when_dry() {
        assert_eq!(w().brew_fraction(), 0.0);
    }

    #[test]
    fn brew_fraction_half_at_midpoint() {
        let mut w = w();
        w.brew = 50.0;
        assert!((w.brew_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_potency_zero_when_dry() {
        assert_eq!(w().effective_potency(100.0), 0.0);
    }

    #[test]
    fn effective_potency_scales_with_brew() {
        let mut w = w();
        w.brew = 75.0;
        assert!((w.effective_potency(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_potency_zero_when_disabled() {
        let mut w = w();
        w.brew = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_potency(100.0), 0.0);
    }
}
