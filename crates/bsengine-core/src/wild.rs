use bevy_ecs::prelude::Component;

/// Feral-instinct accumulation tracker named after wild, the
/// adjective and noun meaning living or growing in the natural
/// environment; not domesticated, cultivated, or tamed; marked
/// by strong, uncontrolled emotions or behavior; a wild place
/// or uninhabited region — from the Old English wilde (wild,
/// undomesticated, uncultivated, desolate), from the Proto-
/// Germanic wildijaz (wild), from the Proto-Indo-European root
/// ghwelto- or possibly from the root weld- (forest). The
/// opposed pair tame/wild organizes one of the oldest conceptual
/// boundaries in human culture: the boundary between what has
/// been brought under human control and what has not, between
/// what has been cultivated and what grows according to its
/// own nature, between the village and the forest. Wild things
/// are things that belong to themselves: they follow their own
/// natures, their own seasons, their own drives, without
/// subordinating these to human purposes. The wild animal is
/// dangerous not because it is malicious but because its
/// behavior is governed by its own instincts rather than by
/// the expectations of the domesticated relationship. In human
/// contexts, to go wild is to release the behaviors that
/// domestication — social training, self-control, cultural
/// norm — normally suppresses: the feral child, the berserker,
/// the person who has lost themselves to passion, all go
/// wild in the sense of releasing what was caged. In game
/// mechanics, a wild mechanic models the accumulation of
/// feral energy — the build of instinct, untamed drive, or
/// primal force that eventually reaches the threshold at which
/// a character sheds the constraints of civilized behavior
/// and gains access to primal, feral, or wild-dependent
/// abilities. `feral` builds via `unleash(amount)` and
/// accumulates passively at `instinct_rate` per second in
/// `tick(dt)` or is tamed via `tame(amount)`.
///
/// Models feral-instinct fill levels, wild-saturation bars,
/// primal-force accumulation trackers, untamed-build gauges,
/// berserker fill levels, instinct-saturation indicators,
/// raw-nature accumulation bars, primal meters, feral-
/// completion fill levels, or any mechanic where a character
/// slowly accumulates the feral energy, primal drive, or
/// wild instinct required to enter a berserker state, access
/// nature-based powers, or reach the threshold of full
/// feral release.
///
/// `unleash(amount)` adds feral; fires `just_wild` when first
/// reaching `max_feral`. No-op when disabled.
///
/// `tame(amount)` reduces feral immediately; fires `just_tame`
/// when reaching 0. No-op when disabled or already tame.
///
/// `tick(dt)` clears both flags, then increases feral by
/// `instinct_rate * dt` (capped at `max_feral`). Fires
/// `just_wild` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_wild()` returns `feral >= max_feral && enabled`.
///
/// `is_tame()` returns `feral == 0.0` (not gated by `enabled`).
///
/// `feral_fraction()` returns `(feral / max_feral).clamp(0, 1)`.
///
/// `effective_primal(scale)` returns `scale * feral_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — unleashes at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wild {
    pub feral: f32,
    pub max_feral: f32,
    pub instinct_rate: f32,
    pub just_wild: bool,
    pub just_tame: bool,
    pub enabled: bool,
}

impl Wild {
    pub fn new(max_feral: f32, instinct_rate: f32) -> Self {
        Self {
            feral: 0.0,
            max_feral: max_feral.max(0.1),
            instinct_rate: instinct_rate.max(0.0),
            just_wild: false,
            just_tame: false,
            enabled: true,
        }
    }

    /// Add feral; fires `just_wild` when first reaching max.
    /// No-op when disabled.
    pub fn unleash(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.feral < self.max_feral;
        self.feral = (self.feral + amount).min(self.max_feral);
        if was_below && self.feral >= self.max_feral {
            self.just_wild = true;
        }
    }

    /// Reduce feral; fires `just_tame` when reaching 0.
    /// No-op when disabled or already tame.
    pub fn tame(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.feral <= 0.0 {
            return;
        }
        self.feral = (self.feral - amount).max(0.0);
        if self.feral <= 0.0 {
            self.just_tame = true;
        }
    }

    /// Clear flags, then increase feral by `instinct_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_wild = false;
        self.just_tame = false;
        if self.enabled && self.instinct_rate > 0.0 && self.feral < self.max_feral {
            let was_below = self.feral < self.max_feral;
            self.feral = (self.feral + self.instinct_rate * dt).min(self.max_feral);
            if was_below && self.feral >= self.max_feral {
                self.just_wild = true;
            }
        }
    }

    /// `true` when feral is at maximum and component is enabled.
    pub fn is_wild(&self) -> bool {
        self.feral >= self.max_feral && self.enabled
    }

    /// `true` when feral is 0 (not gated by `enabled`).
    pub fn is_tame(&self) -> bool {
        self.feral == 0.0
    }

    /// Fraction of maximum feral [0.0, 1.0].
    pub fn feral_fraction(&self) -> f32 {
        (self.feral / self.max_feral).clamp(0.0, 1.0)
    }

    /// Returns `scale * feral_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_primal(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.feral_fraction()
    }
}

impl Default for Wild {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wild {
        Wild::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_tame() {
        let w = w();
        assert_eq!(w.feral, 0.0);
        assert!(w.is_tame());
        assert!(!w.is_wild());
    }

    #[test]
    fn new_clamps_max_feral() {
        let w = Wild::new(-5.0, 1.5);
        assert!((w.max_feral - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_instinct_rate() {
        let w = Wild::new(100.0, -1.5);
        assert_eq!(w.instinct_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Wild::default();
        assert!((w.max_feral - 100.0).abs() < 1e-5);
        assert!((w.instinct_rate - 1.5).abs() < 1e-5);
    }

    // --- unleash ---

    #[test]
    fn unleash_adds_feral() {
        let mut w = w();
        w.unleash(40.0);
        assert!((w.feral - 40.0).abs() < 1e-3);
    }

    #[test]
    fn unleash_clamps_at_max() {
        let mut w = w();
        w.unleash(200.0);
        assert!((w.feral - 100.0).abs() < 1e-3);
    }

    #[test]
    fn unleash_fires_just_wild_at_max() {
        let mut w = w();
        w.unleash(100.0);
        assert!(w.just_wild);
        assert!(w.is_wild());
    }

    #[test]
    fn unleash_no_just_wild_when_already_at_max() {
        let mut w = w();
        w.feral = 100.0;
        w.unleash(10.0);
        assert!(!w.just_wild);
    }

    #[test]
    fn unleash_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.unleash(50.0);
        assert_eq!(w.feral, 0.0);
    }

    #[test]
    fn unleash_no_op_when_amount_zero() {
        let mut w = w();
        w.unleash(0.0);
        assert_eq!(w.feral, 0.0);
    }

    // --- tame ---

    #[test]
    fn tame_reduces_feral() {
        let mut w = w();
        w.feral = 60.0;
        w.tame(20.0);
        assert!((w.feral - 40.0).abs() < 1e-3);
    }

    #[test]
    fn tame_clamps_at_zero() {
        let mut w = w();
        w.feral = 30.0;
        w.tame(200.0);
        assert_eq!(w.feral, 0.0);
    }

    #[test]
    fn tame_fires_just_tame_at_zero() {
        let mut w = w();
        w.feral = 30.0;
        w.tame(30.0);
        assert!(w.just_tame);
    }

    #[test]
    fn tame_no_op_when_already_tame() {
        let mut w = w();
        w.tame(10.0);
        assert!(!w.just_tame);
    }

    #[test]
    fn tame_no_op_when_disabled() {
        let mut w = w();
        w.feral = 50.0;
        w.enabled = false;
        w.tame(50.0);
        assert!((w.feral - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_feral() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.feral - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_wild_on_feral_to_max() {
        let mut w = Wild::new(100.0, 200.0);
        w.feral = 95.0;
        w.tick(1.0);
        assert!(w.just_wild);
        assert!(w.is_wild());
    }

    #[test]
    fn tick_no_build_when_already_wild() {
        let mut w = w();
        w.feral = 100.0;
        w.tick(1.0);
        assert!(!w.just_wild);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Wild::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.feral, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.feral, 0.0);
    }

    #[test]
    fn tick_clears_just_wild() {
        let mut w = Wild::new(100.0, 200.0);
        w.feral = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_wild);
    }

    #[test]
    fn tick_clears_just_tame() {
        let mut w = w();
        w.feral = 10.0;
        w.tame(10.0);
        w.tick(0.016);
        assert!(!w.just_tame);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.feral - 9.0).abs() < 1e-3);
    }

    // --- is_wild / is_tame ---

    #[test]
    fn is_wild_false_when_disabled() {
        let mut w = w();
        w.feral = 100.0;
        w.enabled = false;
        assert!(!w.is_wild());
    }

    #[test]
    fn is_tame_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_tame());
    }

    // --- feral_fraction / effective_primal ---

    #[test]
    fn feral_fraction_zero_when_tame() {
        assert_eq!(w().feral_fraction(), 0.0);
    }

    #[test]
    fn feral_fraction_half_at_midpoint() {
        let mut w = w();
        w.feral = 50.0;
        assert!((w.feral_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_primal_zero_when_tame() {
        assert_eq!(w().effective_primal(100.0), 0.0);
    }

    #[test]
    fn effective_primal_scales_with_feral() {
        let mut w = w();
        w.feral = 75.0;
        assert!((w.effective_primal(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_primal_zero_when_disabled() {
        let mut w = w();
        w.feral = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_primal(100.0), 0.0);
    }
}
