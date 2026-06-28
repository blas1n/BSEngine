use bevy_ecs::prelude::Component;

/// Corruption-temptation accumulation tracker named after vice, the
/// noun that arrived in English in the thirteenth century through
/// Old French vice from the Latin vitium — a blemish, a defect, a
/// fault — and has since expanded its semantic range from the
/// relatively neutral sense of a physical imperfection to the moral
/// sense of a habitual tendency toward evil, a settled disposition
/// that pulls its possessor repeatedly toward the same class of
/// transgression despite the accumulated evidence that the
/// transgression is harmful. In scholastic moral theology, vice is
/// the corruption of the faculty that vice exercises: the vice of
/// lust corrupts the faculty of desire; the vice of pride corrupts
/// the faculty of self-assessment; the vice of sloth corrupts the
/// faculty of moral effort. Thomas Aquinas argued that vices are
/// habits — hexeis in Aristotle's terminology — acquired through
/// repetition: each act of lust makes the next act of lust easier,
/// each act of pride makes the next act of pride more reflexive,
/// until the faculty in question is so thoroughly formed by its
/// habitual misuse that the disordered act becomes the path of
/// least resistance. In English legal tradition, vice also names
/// moral offences that fall short of statutory crime but are
/// regulated by social pressure: gambling, prostitution, and drug
/// use have all at various historical moments been classed as vices
/// — socially condemned, sometimes legally restricted, but primarily
/// understood as failures of individual character rather than
/// violations of public order. In natural language, a vice can also
/// be simply a bad habit — eating too fast, biting one's nails,
/// checking a phone compulsively — which preserves the sense of
/// habitual repetition while evacuating the moral gravity. In game
/// mechanics, vice is the accumulation of corruption that grows with
/// each temptation yielded to and that shapes the character's
/// available options, narrative standing, and eventual fate. `corruption`
/// builds via `corrupt(amount)` and accumulates passively at
/// `temptation_rate` per second in `tick(dt)` or is cleansed via
/// `purify(amount)`.
///
/// Models corruption-fill levels, temptation-saturation bars,
/// moral-degradation accumulators, vice-accumulation gauges,
/// sin-debt fill levels, depravity-saturation indicators,
/// addiction-habit accumulation bars, ethical-erosion meters,
/// reputation-corruption fill levels, or any mechanic where a
/// character, faction, or institution slowly accumulates the weight
/// of its habitual misdeeds — each yielded temptation making the
/// next slightly harder to resist — until the corruption is total
/// and redemption, if it remains possible at all, requires an
/// extraordinary act of renunciation that reverses a trajectory
/// built up over many compounding choices.
///
/// `corrupt(amount)` adds corruption; fires `just_corrupted` when
/// first reaching `max_corruption`. No-op when disabled.
///
/// `purify(amount)` reduces corruption immediately; fires
/// `just_purified` when reaching 0. No-op when disabled or already
/// pure.
///
/// `tick(dt)` clears both flags, then increases corruption by
/// `temptation_rate * dt` (capped at `max_corruption`). Fires
/// `just_corrupted` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_corrupted()` returns `corruption >= max_corruption && enabled`.
///
/// `is_purified()` returns `corruption == 0.0` (not gated by
/// `enabled`).
///
/// `corruption_fraction()` returns
/// `(corruption / max_corruption).clamp(0, 1)`.
///
/// `effective_depravity(scale)` returns `scale * corruption_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — tempts at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Vice {
    pub corruption: f32,
    pub max_corruption: f32,
    pub temptation_rate: f32,
    pub just_corrupted: bool,
    pub just_purified: bool,
    pub enabled: bool,
}

impl Vice {
    pub fn new(max_corruption: f32, temptation_rate: f32) -> Self {
        Self {
            corruption: 0.0,
            max_corruption: max_corruption.max(0.1),
            temptation_rate: temptation_rate.max(0.0),
            just_corrupted: false,
            just_purified: false,
            enabled: true,
        }
    }

    /// Add corruption; fires `just_corrupted` when first reaching max.
    /// No-op when disabled.
    pub fn corrupt(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.corruption < self.max_corruption;
        self.corruption = (self.corruption + amount).min(self.max_corruption);
        if was_below && self.corruption >= self.max_corruption {
            self.just_corrupted = true;
        }
    }

    /// Reduce corruption; fires `just_purified` when reaching 0.
    /// No-op when disabled or already pure.
    pub fn purify(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.corruption <= 0.0 {
            return;
        }
        self.corruption = (self.corruption - amount).max(0.0);
        if self.corruption <= 0.0 {
            self.just_purified = true;
        }
    }

    /// Clear flags, then increase corruption by `temptation_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_corrupted = false;
        self.just_purified = false;
        if self.enabled && self.temptation_rate > 0.0 && self.corruption < self.max_corruption {
            let was_below = self.corruption < self.max_corruption;
            self.corruption =
                (self.corruption + self.temptation_rate * dt).min(self.max_corruption);
            if was_below && self.corruption >= self.max_corruption {
                self.just_corrupted = true;
            }
        }
    }

    /// `true` when corruption is at maximum and component is enabled.
    pub fn is_corrupted(&self) -> bool {
        self.corruption >= self.max_corruption && self.enabled
    }

    /// `true` when corruption is 0 (not gated by `enabled`).
    pub fn is_purified(&self) -> bool {
        self.corruption == 0.0
    }

    /// Fraction of maximum corruption [0.0, 1.0].
    pub fn corruption_fraction(&self) -> f32 {
        (self.corruption / self.max_corruption).clamp(0.0, 1.0)
    }

    /// Returns `scale * corruption_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_depravity(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.corruption_fraction()
    }
}

impl Default for Vice {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v() -> Vice {
        Vice::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_purified() {
        let v = v();
        assert_eq!(v.corruption, 0.0);
        assert!(v.is_purified());
        assert!(!v.is_corrupted());
    }

    #[test]
    fn new_clamps_max_corruption() {
        let v = Vice::new(-5.0, 1.5);
        assert!((v.max_corruption - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_temptation_rate() {
        let v = Vice::new(100.0, -1.5);
        assert_eq!(v.temptation_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let v = Vice::default();
        assert!((v.max_corruption - 100.0).abs() < 1e-5);
        assert!((v.temptation_rate - 1.5).abs() < 1e-5);
    }

    // --- corrupt ---

    #[test]
    fn corrupt_adds_corruption() {
        let mut v = v();
        v.corrupt(40.0);
        assert!((v.corruption - 40.0).abs() < 1e-3);
    }

    #[test]
    fn corrupt_clamps_at_max() {
        let mut v = v();
        v.corrupt(200.0);
        assert!((v.corruption - 100.0).abs() < 1e-3);
    }

    #[test]
    fn corrupt_fires_just_corrupted_at_max() {
        let mut v = v();
        v.corrupt(100.0);
        assert!(v.just_corrupted);
        assert!(v.is_corrupted());
    }

    #[test]
    fn corrupt_no_just_corrupted_when_already_at_max() {
        let mut v = v();
        v.corruption = 100.0;
        v.corrupt(10.0);
        assert!(!v.just_corrupted);
    }

    #[test]
    fn corrupt_no_op_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.corrupt(50.0);
        assert_eq!(v.corruption, 0.0);
    }

    #[test]
    fn corrupt_no_op_when_amount_zero() {
        let mut v = v();
        v.corrupt(0.0);
        assert_eq!(v.corruption, 0.0);
    }

    // --- purify ---

    #[test]
    fn purify_reduces_corruption() {
        let mut v = v();
        v.corruption = 60.0;
        v.purify(20.0);
        assert!((v.corruption - 40.0).abs() < 1e-3);
    }

    #[test]
    fn purify_clamps_at_zero() {
        let mut v = v();
        v.corruption = 30.0;
        v.purify(200.0);
        assert_eq!(v.corruption, 0.0);
    }

    #[test]
    fn purify_fires_just_purified_at_zero() {
        let mut v = v();
        v.corruption = 30.0;
        v.purify(30.0);
        assert!(v.just_purified);
    }

    #[test]
    fn purify_no_op_when_already_purified() {
        let mut v = v();
        v.purify(10.0);
        assert!(!v.just_purified);
    }

    #[test]
    fn purify_no_op_when_disabled() {
        let mut v = v();
        v.corruption = 50.0;
        v.enabled = false;
        v.purify(50.0);
        assert!((v.corruption - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_corruption() {
        let mut v = v(); // rate=1.5
        v.tick(4.0); // 0 + 1.5*4 = 6
        assert!((v.corruption - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_corrupted_on_corruption_to_max() {
        let mut v = Vice::new(100.0, 200.0);
        v.corruption = 95.0;
        v.tick(1.0);
        assert!(v.just_corrupted);
        assert!(v.is_corrupted());
    }

    #[test]
    fn tick_no_build_when_already_corrupted() {
        let mut v = v();
        v.corruption = 100.0;
        v.tick(1.0);
        assert!(!v.just_corrupted);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut v = Vice::new(100.0, 0.0);
        v.tick(100.0);
        assert_eq!(v.corruption, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.tick(1.0);
        assert_eq!(v.corruption, 0.0);
    }

    #[test]
    fn tick_clears_just_corrupted() {
        let mut v = Vice::new(100.0, 200.0);
        v.corruption = 95.0;
        v.tick(1.0);
        v.tick(0.016);
        assert!(!v.just_corrupted);
    }

    #[test]
    fn tick_clears_just_purified() {
        let mut v = v();
        v.corruption = 10.0;
        v.purify(10.0);
        v.tick(0.016);
        assert!(!v.just_purified);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut v = v(); // rate=1.5
        v.tick(6.0); // 1.5*6 = 9
        assert!((v.corruption - 9.0).abs() < 1e-3);
    }

    // --- is_corrupted / is_purified ---

    #[test]
    fn is_corrupted_false_when_disabled() {
        let mut v = v();
        v.corruption = 100.0;
        v.enabled = false;
        assert!(!v.is_corrupted());
    }

    #[test]
    fn is_purified_not_gated_by_enabled() {
        let mut v = v();
        v.enabled = false;
        assert!(v.is_purified());
    }

    // --- corruption_fraction / effective_depravity ---

    #[test]
    fn corruption_fraction_zero_when_purified() {
        assert_eq!(v().corruption_fraction(), 0.0);
    }

    #[test]
    fn corruption_fraction_half_at_midpoint() {
        let mut v = v();
        v.corruption = 50.0;
        assert!((v.corruption_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_depravity_zero_when_purified() {
        assert_eq!(v().effective_depravity(100.0), 0.0);
    }

    #[test]
    fn effective_depravity_scales_with_corruption() {
        let mut v = v();
        v.corruption = 75.0;
        assert!((v.effective_depravity(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_depravity_zero_when_disabled() {
        let mut v = v();
        v.corruption = 50.0;
        v.enabled = false;
        assert_eq!(v.effective_depravity(100.0), 0.0);
    }
}
