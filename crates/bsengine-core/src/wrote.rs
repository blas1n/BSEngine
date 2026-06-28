use bevy_ecs::prelude::Component;

/// Legacy-corpus accumulation tracker named after wrote, the
/// past tense of write — meaning to have composed, authored,
/// or inscribed; the state of having produced written works;
/// the completed act of writing — from the Old English wrāt
/// (past tense of wrītan, to write), from the Proto-Germanic
/// wraitō (a scratch, a mark, a written sign), from the
/// Proto-Indo-European root wreid- (to tear, to scratch).
/// Where write names the act in the present — the motion of
/// pen on paper, stylus on clay, finger on glass — wrote
/// names the completed act, the work left behind, the corpus
/// that persists when the hand has lifted. Wrote is what
/// remains: the letter that has been sent cannot be unsent;
/// the book that has been written cannot be unwritten;
/// the inscription that has been carved into stone cannot
/// be uncarved. The past tense of writing is, therefore,
/// the permanent record — the trace of will and effort and
/// attention that survives the writer. In biographical usage,
/// wrote functions as the organizing verb of a literary
/// life: she wrote thirty novels; he wrote nothing after
/// the accident; they wrote together under a shared name.
/// To enumerate what someone wrote is to take the measure
/// of a life in its written traces, its contributions to
/// the record, its legacy in accumulated text. In game
/// mechanics, a wrote mechanic models the accumulation of
/// authored legacy — the build of completed works, inscribed
/// records, composed messages, or finished texts that
/// eventually reaches the threshold at which a corpus is
/// complete, a reputation is established, or a legacy is
/// secured. `legacy` builds via `compose(amount)` and
/// accumulates passively at `prose_rate` per second in
/// `tick(dt)` or is erased via `delete(amount)`.
///
/// Models legacy-corpus fill levels, authorship-saturation
/// bars, work-accumulation trackers, opus-build gauges,
/// reputation-fill levels, oeuvre-saturation indicators,
/// canon-accumulation bars, corpus meters, legacy-completion
/// fill levels, or any mechanic where a character, scholar,
/// or entity slowly accumulates the completed works, inscribed
/// records, or authored texts that establish their reputation,
/// secure their legacy, or fill the threshold of a complete
/// literary corpus.
///
/// `compose(amount)` adds legacy; fires `just_authored` when
/// first reaching `max_legacy`. No-op when disabled.
///
/// `delete(amount)` reduces legacy immediately; fires
/// `just_erased` when reaching 0. No-op when disabled or
/// already erased.
///
/// `tick(dt)` clears both flags, then increases legacy by
/// `prose_rate * dt` (capped at `max_legacy`). Fires
/// `just_authored` when first reaching max. No-op when
/// disabled or rate is 0.
///
/// `is_authored()` returns `legacy >= max_legacy && enabled`.
///
/// `is_erased()` returns `legacy == 0.0` (not gated by
/// `enabled`).
///
/// `legacy_fraction()` returns
/// `(legacy / max_legacy).clamp(0, 1)`.
///
/// `effective_prose(scale)` returns `scale * legacy_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — composes at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wrote {
    pub legacy: f32,
    pub max_legacy: f32,
    pub prose_rate: f32,
    pub just_authored: bool,
    pub just_erased: bool,
    pub enabled: bool,
}

impl Wrote {
    pub fn new(max_legacy: f32, prose_rate: f32) -> Self {
        Self {
            legacy: 0.0,
            max_legacy: max_legacy.max(0.1),
            prose_rate: prose_rate.max(0.0),
            just_authored: false,
            just_erased: false,
            enabled: true,
        }
    }

    /// Add legacy; fires `just_authored` when first reaching max.
    /// No-op when disabled.
    pub fn compose(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.legacy < self.max_legacy;
        self.legacy = (self.legacy + amount).min(self.max_legacy);
        if was_below && self.legacy >= self.max_legacy {
            self.just_authored = true;
        }
    }

    /// Reduce legacy; fires `just_erased` when reaching 0.
    /// No-op when disabled or already erased.
    pub fn delete(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.legacy <= 0.0 {
            return;
        }
        self.legacy = (self.legacy - amount).max(0.0);
        if self.legacy <= 0.0 {
            self.just_erased = true;
        }
    }

    /// Clear flags, then increase legacy by `prose_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_authored = false;
        self.just_erased = false;
        if self.enabled && self.prose_rate > 0.0 && self.legacy < self.max_legacy {
            let was_below = self.legacy < self.max_legacy;
            self.legacy = (self.legacy + self.prose_rate * dt).min(self.max_legacy);
            if was_below && self.legacy >= self.max_legacy {
                self.just_authored = true;
            }
        }
    }

    /// `true` when legacy is at maximum and component is enabled.
    pub fn is_authored(&self) -> bool {
        self.legacy >= self.max_legacy && self.enabled
    }

    /// `true` when legacy is 0 (not gated by `enabled`).
    pub fn is_erased(&self) -> bool {
        self.legacy == 0.0
    }

    /// Fraction of maximum legacy [0.0, 1.0].
    pub fn legacy_fraction(&self) -> f32 {
        (self.legacy / self.max_legacy).clamp(0.0, 1.0)
    }

    /// Returns `scale * legacy_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_prose(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.legacy_fraction()
    }
}

impl Default for Wrote {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wrote {
        Wrote::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_erased() {
        let w = w();
        assert_eq!(w.legacy, 0.0);
        assert!(w.is_erased());
        assert!(!w.is_authored());
    }

    #[test]
    fn new_clamps_max_legacy() {
        let w = Wrote::new(-5.0, 1.5);
        assert!((w.max_legacy - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_prose_rate() {
        let w = Wrote::new(100.0, -1.5);
        assert_eq!(w.prose_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Wrote::default();
        assert!((w.max_legacy - 100.0).abs() < 1e-5);
        assert!((w.prose_rate - 1.5).abs() < 1e-5);
    }

    // --- compose ---

    #[test]
    fn compose_adds_legacy() {
        let mut w = w();
        w.compose(40.0);
        assert!((w.legacy - 40.0).abs() < 1e-3);
    }

    #[test]
    fn compose_clamps_at_max() {
        let mut w = w();
        w.compose(200.0);
        assert!((w.legacy - 100.0).abs() < 1e-3);
    }

    #[test]
    fn compose_fires_just_authored_at_max() {
        let mut w = w();
        w.compose(100.0);
        assert!(w.just_authored);
        assert!(w.is_authored());
    }

    #[test]
    fn compose_no_just_authored_when_already_at_max() {
        let mut w = w();
        w.legacy = 100.0;
        w.compose(10.0);
        assert!(!w.just_authored);
    }

    #[test]
    fn compose_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.compose(50.0);
        assert_eq!(w.legacy, 0.0);
    }

    #[test]
    fn compose_no_op_when_amount_zero() {
        let mut w = w();
        w.compose(0.0);
        assert_eq!(w.legacy, 0.0);
    }

    // --- delete ---

    #[test]
    fn delete_reduces_legacy() {
        let mut w = w();
        w.legacy = 60.0;
        w.delete(20.0);
        assert!((w.legacy - 40.0).abs() < 1e-3);
    }

    #[test]
    fn delete_clamps_at_zero() {
        let mut w = w();
        w.legacy = 30.0;
        w.delete(200.0);
        assert_eq!(w.legacy, 0.0);
    }

    #[test]
    fn delete_fires_just_erased_at_zero() {
        let mut w = w();
        w.legacy = 30.0;
        w.delete(30.0);
        assert!(w.just_erased);
    }

    #[test]
    fn delete_no_op_when_already_erased() {
        let mut w = w();
        w.delete(10.0);
        assert!(!w.just_erased);
    }

    #[test]
    fn delete_no_op_when_disabled() {
        let mut w = w();
        w.legacy = 50.0;
        w.enabled = false;
        w.delete(50.0);
        assert!((w.legacy - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_legacy() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.legacy - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_authored_on_legacy_to_max() {
        let mut w = Wrote::new(100.0, 200.0);
        w.legacy = 95.0;
        w.tick(1.0);
        assert!(w.just_authored);
        assert!(w.is_authored());
    }

    #[test]
    fn tick_no_build_when_already_authored() {
        let mut w = w();
        w.legacy = 100.0;
        w.tick(1.0);
        assert!(!w.just_authored);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Wrote::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.legacy, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.legacy, 0.0);
    }

    #[test]
    fn tick_clears_just_authored() {
        let mut w = Wrote::new(100.0, 200.0);
        w.legacy = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_authored);
    }

    #[test]
    fn tick_clears_just_erased() {
        let mut w = w();
        w.legacy = 10.0;
        w.delete(10.0);
        w.tick(0.016);
        assert!(!w.just_erased);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.legacy - 9.0).abs() < 1e-3);
    }

    // --- is_authored / is_erased ---

    #[test]
    fn is_authored_false_when_disabled() {
        let mut w = w();
        w.legacy = 100.0;
        w.enabled = false;
        assert!(!w.is_authored());
    }

    #[test]
    fn is_erased_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_erased());
    }

    // --- legacy_fraction / effective_prose ---

    #[test]
    fn legacy_fraction_zero_when_erased() {
        assert_eq!(w().legacy_fraction(), 0.0);
    }

    #[test]
    fn legacy_fraction_half_at_midpoint() {
        let mut w = w();
        w.legacy = 50.0;
        assert!((w.legacy_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_prose_zero_when_erased() {
        assert_eq!(w().effective_prose(100.0), 0.0);
    }

    #[test]
    fn effective_prose_scales_with_legacy() {
        let mut w = w();
        w.legacy = 75.0;
        assert!((w.effective_prose(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_prose_zero_when_disabled() {
        let mut w = w();
        w.legacy = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_prose(100.0), 0.0);
    }
}
