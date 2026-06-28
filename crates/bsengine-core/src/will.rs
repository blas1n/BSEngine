use bevy_ecs::prelude::Component;

/// Willpower-determination accumulation tracker named after will,
/// the noun and verb meaning the mental faculty by which one
/// deliberately chooses or decides upon a course of action;
/// the power of control the mind has over itself; firm determination
/// or resolution; to intend or decide — from the Old English
/// willan, wyllan (to desire, to wish, to want), from the Proto-
/// Germanic *wiljaną (to want, to wish), from the Proto-Indo-
/// European root wel- (to wish, to will). The root wel- is
/// extraordinarily productive: it gave Latin volo (I wish, I am
/// willing), voluntas (will, desire), and the derived voluntary,
/// volition, benevolent (good-wishing), and malevolent (ill-wishing).
/// It gave the Germanic languages well (in a manner that pleases
/// one's will), wealth (the condition of having one's will
/// fulfilled), and will itself — the capacity from which all
/// wishing proceeds. In philosophy of action, the will is the
/// mysterious bridge between intention and motion: the faculty
/// that converts a desire into an act, that makes the mental
/// event of "I want to move my arm" become the physical event
/// of the arm moving. Kant placed the will at the centre of
/// moral philosophy: the good will is the only thing good without
/// qualification, because it is the faculty of acting according
/// to principle rather than desire. Schopenhauer described the
/// world as Will and Representation — the will as the blind,
/// striving force underlying all existence, visible in the
/// organism's hunger, the plant's growth, the crystal's formation,
/// as well as in human desire and action. In game mechanics, a
/// will mechanic models the slow accumulation of resolve —
/// the build of determination, mental fortitude, or willpower
/// that eventually reaches a threshold at which something becomes
/// possible that was impossible before. `resolve` builds via
/// `focus(amount)` and accumulates passively at `resolve_rate`
/// per second in `tick(dt)` or diminishes via `waver(amount)`.
///
/// Models willpower-determination fill levels, resolve-saturation
/// bars, mental-fortitude accumulators, determination-build
/// gauges, conviction-approach fill levels, focus-saturation
/// indicators, concentration-build accumulation bars, psychic-
/// endurance meters, volition-completion fill levels, or any
/// mechanic where a character, faction, or entity slowly builds
/// the mental strength, determination, or willpower required
/// to resist a temptation, complete a challenge, resist a curse,
/// or simply do the next difficult thing in a world that keeps
/// making difficult things harder.
///
/// `focus(amount)` adds resolve; fires `just_resolved` when
/// first reaching `max_resolve`. No-op when disabled.
///
/// `waver(amount)` reduces resolve immediately; fires
/// `just_broken` when reaching 0. No-op when disabled or
/// already broken.
///
/// `tick(dt)` clears both flags, then increases resolve by
/// `resolve_rate * dt` (capped at `max_resolve`). Fires
/// `just_resolved` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_resolved()` returns `resolve >= max_resolve && enabled`.
///
/// `is_broken()` returns `resolve == 0.0` (not gated by `enabled`).
///
/// `resolve_fraction()` returns
/// `(resolve / max_resolve).clamp(0, 1)`.
///
/// `effective_will(scale)` returns `scale * resolve_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — resolves at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Will {
    pub resolve: f32,
    pub max_resolve: f32,
    pub resolve_rate: f32,
    pub just_resolved: bool,
    pub just_broken: bool,
    pub enabled: bool,
}

impl Will {
    pub fn new(max_resolve: f32, resolve_rate: f32) -> Self {
        Self {
            resolve: 0.0,
            max_resolve: max_resolve.max(0.1),
            resolve_rate: resolve_rate.max(0.0),
            just_resolved: false,
            just_broken: false,
            enabled: true,
        }
    }

    /// Add resolve; fires `just_resolved` when first reaching max.
    /// No-op when disabled.
    pub fn focus(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.resolve < self.max_resolve;
        self.resolve = (self.resolve + amount).min(self.max_resolve);
        if was_below && self.resolve >= self.max_resolve {
            self.just_resolved = true;
        }
    }

    /// Reduce resolve; fires `just_broken` when reaching 0.
    /// No-op when disabled or already broken.
    pub fn waver(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.resolve <= 0.0 {
            return;
        }
        self.resolve = (self.resolve - amount).max(0.0);
        if self.resolve <= 0.0 {
            self.just_broken = true;
        }
    }

    /// Clear flags, then increase resolve by `resolve_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_resolved = false;
        self.just_broken = false;
        if self.enabled && self.resolve_rate > 0.0 && self.resolve < self.max_resolve {
            let was_below = self.resolve < self.max_resolve;
            self.resolve = (self.resolve + self.resolve_rate * dt).min(self.max_resolve);
            if was_below && self.resolve >= self.max_resolve {
                self.just_resolved = true;
            }
        }
    }

    /// `true` when resolve is at maximum and component is enabled.
    pub fn is_resolved(&self) -> bool {
        self.resolve >= self.max_resolve && self.enabled
    }

    /// `true` when resolve is 0 (not gated by `enabled`).
    pub fn is_broken(&self) -> bool {
        self.resolve == 0.0
    }

    /// Fraction of maximum resolve [0.0, 1.0].
    pub fn resolve_fraction(&self) -> f32 {
        (self.resolve / self.max_resolve).clamp(0.0, 1.0)
    }

    /// Returns `scale * resolve_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_will(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.resolve_fraction()
    }
}

impl Default for Will {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Will {
        Will::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_broken() {
        let w = w();
        assert_eq!(w.resolve, 0.0);
        assert!(w.is_broken());
        assert!(!w.is_resolved());
    }

    #[test]
    fn new_clamps_max_resolve() {
        let w = Will::new(-5.0, 1.5);
        assert!((w.max_resolve - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_resolve_rate() {
        let w = Will::new(100.0, -1.5);
        assert_eq!(w.resolve_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Will::default();
        assert!((w.max_resolve - 100.0).abs() < 1e-5);
        assert!((w.resolve_rate - 1.5).abs() < 1e-5);
    }

    // --- focus ---

    #[test]
    fn focus_adds_resolve() {
        let mut w = w();
        w.focus(40.0);
        assert!((w.resolve - 40.0).abs() < 1e-3);
    }

    #[test]
    fn focus_clamps_at_max() {
        let mut w = w();
        w.focus(200.0);
        assert!((w.resolve - 100.0).abs() < 1e-3);
    }

    #[test]
    fn focus_fires_just_resolved_at_max() {
        let mut w = w();
        w.focus(100.0);
        assert!(w.just_resolved);
        assert!(w.is_resolved());
    }

    #[test]
    fn focus_no_just_resolved_when_already_at_max() {
        let mut w = w();
        w.resolve = 100.0;
        w.focus(10.0);
        assert!(!w.just_resolved);
    }

    #[test]
    fn focus_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.focus(50.0);
        assert_eq!(w.resolve, 0.0);
    }

    #[test]
    fn focus_no_op_when_amount_zero() {
        let mut w = w();
        w.focus(0.0);
        assert_eq!(w.resolve, 0.0);
    }

    // --- waver ---

    #[test]
    fn waver_reduces_resolve() {
        let mut w = w();
        w.resolve = 60.0;
        w.waver(20.0);
        assert!((w.resolve - 40.0).abs() < 1e-3);
    }

    #[test]
    fn waver_clamps_at_zero() {
        let mut w = w();
        w.resolve = 30.0;
        w.waver(200.0);
        assert_eq!(w.resolve, 0.0);
    }

    #[test]
    fn waver_fires_just_broken_at_zero() {
        let mut w = w();
        w.resolve = 30.0;
        w.waver(30.0);
        assert!(w.just_broken);
    }

    #[test]
    fn waver_no_op_when_already_broken() {
        let mut w = w();
        w.waver(10.0);
        assert!(!w.just_broken);
    }

    #[test]
    fn waver_no_op_when_disabled() {
        let mut w = w();
        w.resolve = 50.0;
        w.enabled = false;
        w.waver(50.0);
        assert!((w.resolve - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_resolve() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.resolve - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_resolved_on_resolve_to_max() {
        let mut w = Will::new(100.0, 200.0);
        w.resolve = 95.0;
        w.tick(1.0);
        assert!(w.just_resolved);
        assert!(w.is_resolved());
    }

    #[test]
    fn tick_no_build_when_already_resolved() {
        let mut w = w();
        w.resolve = 100.0;
        w.tick(1.0);
        assert!(!w.just_resolved);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Will::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.resolve, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.resolve, 0.0);
    }

    #[test]
    fn tick_clears_just_resolved() {
        let mut w = Will::new(100.0, 200.0);
        w.resolve = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_resolved);
    }

    #[test]
    fn tick_clears_just_broken() {
        let mut w = w();
        w.resolve = 10.0;
        w.waver(10.0);
        w.tick(0.016);
        assert!(!w.just_broken);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.resolve - 9.0).abs() < 1e-3);
    }

    // --- is_resolved / is_broken ---

    #[test]
    fn is_resolved_false_when_disabled() {
        let mut w = w();
        w.resolve = 100.0;
        w.enabled = false;
        assert!(!w.is_resolved());
    }

    #[test]
    fn is_broken_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_broken());
    }

    // --- resolve_fraction / effective_will ---

    #[test]
    fn resolve_fraction_zero_when_broken() {
        assert_eq!(w().resolve_fraction(), 0.0);
    }

    #[test]
    fn resolve_fraction_half_at_midpoint() {
        let mut w = w();
        w.resolve = 50.0;
        assert!((w.resolve_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_will_zero_when_broken() {
        assert_eq!(w().effective_will(100.0), 0.0);
    }

    #[test]
    fn effective_will_scales_with_resolve() {
        let mut w = w();
        w.resolve = 75.0;
        assert!((w.effective_will(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_will_zero_when_disabled() {
        let mut w = w();
        w.resolve = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_will(100.0), 0.0);
    }
}
