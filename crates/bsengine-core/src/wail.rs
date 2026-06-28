use bevy_ecs::prelude::Component;

/// Grief-lamentation accumulation tracker named after wail, the verb
/// and noun meaning to make a long, loud, mournful cry; to express
/// grief or suffering by making such cries — from the Old Norse væla
/// (to wail, to moan), related to the Old English wa (woe, grief),
/// from the Proto-Germanic root wa-, which was itself an exclamatory
/// interjection of pain or distress. The same root gave English woe
/// — the noun form of the cry — so that wail and woe are etymological
/// siblings, the sound and the state respectively. The wail appears
/// across human cultures as the primary acoustic expression of
/// unbearable grief: the keen (Irish traditional lamentation), the
/// ululation (Middle Eastern and North African mourning cry), the
/// Nnenia (Greek threnos), the Japanese yokyoku — all are forms of
/// the wail, the long drawn-out sound that signals to the community
/// that something irreversible has happened and that the loss is
/// real, witnessed, and acknowledged. The wail is socially regulated:
/// in most cultures it is the appropriate response to death and
/// severe loss, and its performance is often a social obligation
/// rather than a private release — the mourners who wail loudest
/// are sometimes hired specialists rather than immediate family.
/// In game mechanics, a wail mechanic models the accumulation of
/// expressed grief, lamentation pressure, or mournful resonance
/// that eventually reaches a threshold at which something breaks
/// through — a curse intensifies, a haunting crystallises, a
/// banshee's wail reaches its full volume and delivers its
/// devastating sonic effect. `grief` builds via `lament(amount)`
/// and accumulates passively at `keen_rate` per second in
/// `tick(dt)` or subsides via `console(amount)`.
///
/// Models grief-lamentation fill levels, mourning-saturation bars,
/// sorrow-accumulation trackers, banshee-wail gauges, ululation-
/// approach fill levels, keening-saturation indicators, lament-
/// resonance accumulation bars, funeral-cry meters, dirge-completion
/// fill levels, or any mechanic where a character, spirit, or
/// faction slowly accumulates the grief and lamentation that
/// eventually peaks in a devastating, reality-altering cry —
/// the wail that stops hearts, cracks stone, or marks a death
/// that has finally been accepted and released into the world.
///
/// `lament(amount)` adds grief; fires `just_wailed` when first
/// reaching `max_grief`. No-op when disabled.
///
/// `console(amount)` reduces grief immediately; fires `just_calmed`
/// when reaching 0. No-op when disabled or already calm.
///
/// `tick(dt)` clears both flags, then increases grief by
/// `keen_rate * dt` (capped at `max_grief`). Fires `just_wailed`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_wailed()` returns `grief >= max_grief && enabled`.
///
/// `is_calm()` returns `grief == 0.0` (not gated by `enabled`).
///
/// `grief_fraction()` returns `(grief / max_grief).clamp(0, 1)`.
///
/// `effective_lament(scale)` returns `scale * grief_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — keens at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wail {
    pub grief: f32,
    pub max_grief: f32,
    pub keen_rate: f32,
    pub just_wailed: bool,
    pub just_calmed: bool,
    pub enabled: bool,
}

impl Wail {
    pub fn new(max_grief: f32, keen_rate: f32) -> Self {
        Self {
            grief: 0.0,
            max_grief: max_grief.max(0.1),
            keen_rate: keen_rate.max(0.0),
            just_wailed: false,
            just_calmed: false,
            enabled: true,
        }
    }

    /// Add grief; fires `just_wailed` when first reaching max.
    /// No-op when disabled.
    pub fn lament(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.grief < self.max_grief;
        self.grief = (self.grief + amount).min(self.max_grief);
        if was_below && self.grief >= self.max_grief {
            self.just_wailed = true;
        }
    }

    /// Reduce grief; fires `just_calmed` when reaching 0.
    /// No-op when disabled or already calm.
    pub fn console(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.grief <= 0.0 {
            return;
        }
        self.grief = (self.grief - amount).max(0.0);
        if self.grief <= 0.0 {
            self.just_calmed = true;
        }
    }

    /// Clear flags, then increase grief by `keen_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_wailed = false;
        self.just_calmed = false;
        if self.enabled && self.keen_rate > 0.0 && self.grief < self.max_grief {
            let was_below = self.grief < self.max_grief;
            self.grief = (self.grief + self.keen_rate * dt).min(self.max_grief);
            if was_below && self.grief >= self.max_grief {
                self.just_wailed = true;
            }
        }
    }

    /// `true` when grief is at maximum and component is enabled.
    pub fn is_wailed(&self) -> bool {
        self.grief >= self.max_grief && self.enabled
    }

    /// `true` when grief is 0 (not gated by `enabled`).
    pub fn is_calm(&self) -> bool {
        self.grief == 0.0
    }

    /// Fraction of maximum grief [0.0, 1.0].
    pub fn grief_fraction(&self) -> f32 {
        (self.grief / self.max_grief).clamp(0.0, 1.0)
    }

    /// Returns `scale * grief_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_lament(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.grief_fraction()
    }
}

impl Default for Wail {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wail {
        Wail::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_calm() {
        let w = w();
        assert_eq!(w.grief, 0.0);
        assert!(w.is_calm());
        assert!(!w.is_wailed());
    }

    #[test]
    fn new_clamps_max_grief() {
        let w = Wail::new(-5.0, 1.5);
        assert!((w.max_grief - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_keen_rate() {
        let w = Wail::new(100.0, -1.5);
        assert_eq!(w.keen_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Wail::default();
        assert!((w.max_grief - 100.0).abs() < 1e-5);
        assert!((w.keen_rate - 1.5).abs() < 1e-5);
    }

    // --- lament ---

    #[test]
    fn lament_adds_grief() {
        let mut w = w();
        w.lament(40.0);
        assert!((w.grief - 40.0).abs() < 1e-3);
    }

    #[test]
    fn lament_clamps_at_max() {
        let mut w = w();
        w.lament(200.0);
        assert!((w.grief - 100.0).abs() < 1e-3);
    }

    #[test]
    fn lament_fires_just_wailed_at_max() {
        let mut w = w();
        w.lament(100.0);
        assert!(w.just_wailed);
        assert!(w.is_wailed());
    }

    #[test]
    fn lament_no_just_wailed_when_already_at_max() {
        let mut w = w();
        w.grief = 100.0;
        w.lament(10.0);
        assert!(!w.just_wailed);
    }

    #[test]
    fn lament_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.lament(50.0);
        assert_eq!(w.grief, 0.0);
    }

    #[test]
    fn lament_no_op_when_amount_zero() {
        let mut w = w();
        w.lament(0.0);
        assert_eq!(w.grief, 0.0);
    }

    // --- console ---

    #[test]
    fn console_reduces_grief() {
        let mut w = w();
        w.grief = 60.0;
        w.console(20.0);
        assert!((w.grief - 40.0).abs() < 1e-3);
    }

    #[test]
    fn console_clamps_at_zero() {
        let mut w = w();
        w.grief = 30.0;
        w.console(200.0);
        assert_eq!(w.grief, 0.0);
    }

    #[test]
    fn console_fires_just_calmed_at_zero() {
        let mut w = w();
        w.grief = 30.0;
        w.console(30.0);
        assert!(w.just_calmed);
    }

    #[test]
    fn console_no_op_when_already_calm() {
        let mut w = w();
        w.console(10.0);
        assert!(!w.just_calmed);
    }

    #[test]
    fn console_no_op_when_disabled() {
        let mut w = w();
        w.grief = 50.0;
        w.enabled = false;
        w.console(50.0);
        assert!((w.grief - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_grief() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.grief - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_wailed_on_grief_to_max() {
        let mut w = Wail::new(100.0, 200.0);
        w.grief = 95.0;
        w.tick(1.0);
        assert!(w.just_wailed);
        assert!(w.is_wailed());
    }

    #[test]
    fn tick_no_build_when_already_wailed() {
        let mut w = w();
        w.grief = 100.0;
        w.tick(1.0);
        assert!(!w.just_wailed);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Wail::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.grief, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.grief, 0.0);
    }

    #[test]
    fn tick_clears_just_wailed() {
        let mut w = Wail::new(100.0, 200.0);
        w.grief = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_wailed);
    }

    #[test]
    fn tick_clears_just_calmed() {
        let mut w = w();
        w.grief = 10.0;
        w.console(10.0);
        w.tick(0.016);
        assert!(!w.just_calmed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.grief - 9.0).abs() < 1e-3);
    }

    // --- is_wailed / is_calm ---

    #[test]
    fn is_wailed_false_when_disabled() {
        let mut w = w();
        w.grief = 100.0;
        w.enabled = false;
        assert!(!w.is_wailed());
    }

    #[test]
    fn is_calm_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_calm());
    }

    // --- grief_fraction / effective_lament ---

    #[test]
    fn grief_fraction_zero_when_calm() {
        assert_eq!(w().grief_fraction(), 0.0);
    }

    #[test]
    fn grief_fraction_half_at_midpoint() {
        let mut w = w();
        w.grief = 50.0;
        assert!((w.grief_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_lament_zero_when_calm() {
        assert_eq!(w().effective_lament(100.0), 0.0);
    }

    #[test]
    fn effective_lament_scales_with_grief() {
        let mut w = w();
        w.grief = 75.0;
        assert!((w.effective_lament(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_lament_zero_when_disabled() {
        let mut w = w();
        w.grief = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_lament(100.0), 0.0);
    }
}
