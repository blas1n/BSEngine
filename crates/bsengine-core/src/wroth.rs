use bevy_ecs::prelude::Component;

/// Wrath-fury accumulation tracker named after wroth, the
/// adjective meaning intensely angry; full of wrath; wrathful
/// — from the Old English wrāth (angry, hostile, fierce),
/// from the Proto-Germanic wraiþaz (angry, harsh), from the
/// Proto-Indo-European root wreid- (to turn, to twist), the
/// same root that gave wring, wry, and writhe. The physical
/// metaphor is of something twisted, bent, distorted — a face
/// twisted in fury, a disposition bent out of its ordinary
/// shape by violent emotion. Wroth is the archaic and literary
/// adjective corresponding to the noun wrath: where wrath
/// names the emotion in the abstract, wroth describes a person
/// or deity in the grip of it. The distinction is stylistic:
/// wrath belongs to theology, law, and rhetoric; wroth to
/// poetry, scripture, and elevated narrative. In biblical and
/// epic usage, wroth is how gods describe themselves and how
/// heroes are described at moments of apocalyptic anger —
/// the Iliad's gods grow wroth; the God of the Old Testament
/// is wroth with Israel; the wroth hero is one whose anger
/// has reached the pitch at which it will reshape events.
/// The cooling of wroth is the return to order: a king who
/// was wroth with his vassal is pacified; a deity's wroth
/// is propitiated by sacrifice; a hero's wroth is calmed by
/// the resolution of the insult or injury that provoked it.
/// In game mechanics, a wroth mechanic models the accumulation
/// of righteous or violent anger — the build of fury that
/// eventually reaches the threshold at which a character enters
/// a rage state, gains combat bonuses, or triggers wrath-
/// based effects. `wrath` builds via `seethe(amount)` and
/// accumulates passively at `fury_rate` per second in
/// `tick(dt)` or cools via `pacify(amount)`.
///
/// Models wrath-fury fill levels, anger-saturation bars,
/// rage-accumulation trackers, fury-build gauges, berserker-
/// fill levels, heat-saturation indicators, wroth-accumulation
/// bars, divine-anger meters, fury-completion fill levels,
/// or any mechanic where a character, deity, or entity slowly
/// accumulates the wrath, fury, or righteous anger required
/// to enter a rage state, unleash a devastating attack, or
/// trigger a wrath-based consequence.
///
/// `seethe(amount)` adds wrath; fires `just_wroth` when first
/// reaching `max_wrath`. No-op when disabled.
///
/// `pacify(amount)` reduces wrath immediately; fires
/// `just_calm` when reaching 0. No-op when disabled or
/// already calm.
///
/// `tick(dt)` clears both flags, then increases wrath by
/// `fury_rate * dt` (capped at `max_wrath`). Fires
/// `just_wroth` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_wroth()` returns `wrath >= max_wrath && enabled`.
///
/// `is_calm()` returns `wrath == 0.0` (not gated by
/// `enabled`).
///
/// `wrath_fraction()` returns
/// `(wrath / max_wrath).clamp(0, 1)`.
///
/// `effective_rage(scale)` returns `scale * wrath_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — seethes at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wroth {
    pub wrath: f32,
    pub max_wrath: f32,
    pub fury_rate: f32,
    pub just_wroth: bool,
    pub just_calm: bool,
    pub enabled: bool,
}

impl Wroth {
    pub fn new(max_wrath: f32, fury_rate: f32) -> Self {
        Self {
            wrath: 0.0,
            max_wrath: max_wrath.max(0.1),
            fury_rate: fury_rate.max(0.0),
            just_wroth: false,
            just_calm: false,
            enabled: true,
        }
    }

    /// Add wrath; fires `just_wroth` when first reaching max.
    /// No-op when disabled.
    pub fn seethe(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.wrath < self.max_wrath;
        self.wrath = (self.wrath + amount).min(self.max_wrath);
        if was_below && self.wrath >= self.max_wrath {
            self.just_wroth = true;
        }
    }

    /// Reduce wrath; fires `just_calm` when reaching 0.
    /// No-op when disabled or already calm.
    pub fn pacify(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.wrath <= 0.0 {
            return;
        }
        self.wrath = (self.wrath - amount).max(0.0);
        if self.wrath <= 0.0 {
            self.just_calm = true;
        }
    }

    /// Clear flags, then increase wrath by `fury_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_wroth = false;
        self.just_calm = false;
        if self.enabled && self.fury_rate > 0.0 && self.wrath < self.max_wrath {
            let was_below = self.wrath < self.max_wrath;
            self.wrath = (self.wrath + self.fury_rate * dt).min(self.max_wrath);
            if was_below && self.wrath >= self.max_wrath {
                self.just_wroth = true;
            }
        }
    }

    /// `true` when wrath is at maximum and component is enabled.
    pub fn is_wroth(&self) -> bool {
        self.wrath >= self.max_wrath && self.enabled
    }

    /// `true` when wrath is 0 (not gated by `enabled`).
    pub fn is_calm(&self) -> bool {
        self.wrath == 0.0
    }

    /// Fraction of maximum wrath [0.0, 1.0].
    pub fn wrath_fraction(&self) -> f32 {
        (self.wrath / self.max_wrath).clamp(0.0, 1.0)
    }

    /// Returns `scale * wrath_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_rage(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.wrath_fraction()
    }
}

impl Default for Wroth {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wroth {
        Wroth::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_calm() {
        let w = w();
        assert_eq!(w.wrath, 0.0);
        assert!(w.is_calm());
        assert!(!w.is_wroth());
    }

    #[test]
    fn new_clamps_max_wrath() {
        let w = Wroth::new(-5.0, 1.5);
        assert!((w.max_wrath - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_fury_rate() {
        let w = Wroth::new(100.0, -1.5);
        assert_eq!(w.fury_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Wroth::default();
        assert!((w.max_wrath - 100.0).abs() < 1e-5);
        assert!((w.fury_rate - 1.5).abs() < 1e-5);
    }

    // --- seethe ---

    #[test]
    fn seethe_adds_wrath() {
        let mut w = w();
        w.seethe(40.0);
        assert!((w.wrath - 40.0).abs() < 1e-3);
    }

    #[test]
    fn seethe_clamps_at_max() {
        let mut w = w();
        w.seethe(200.0);
        assert!((w.wrath - 100.0).abs() < 1e-3);
    }

    #[test]
    fn seethe_fires_just_wroth_at_max() {
        let mut w = w();
        w.seethe(100.0);
        assert!(w.just_wroth);
        assert!(w.is_wroth());
    }

    #[test]
    fn seethe_no_just_wroth_when_already_at_max() {
        let mut w = w();
        w.wrath = 100.0;
        w.seethe(10.0);
        assert!(!w.just_wroth);
    }

    #[test]
    fn seethe_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.seethe(50.0);
        assert_eq!(w.wrath, 0.0);
    }

    #[test]
    fn seethe_no_op_when_amount_zero() {
        let mut w = w();
        w.seethe(0.0);
        assert_eq!(w.wrath, 0.0);
    }

    // --- pacify ---

    #[test]
    fn pacify_reduces_wrath() {
        let mut w = w();
        w.wrath = 60.0;
        w.pacify(20.0);
        assert!((w.wrath - 40.0).abs() < 1e-3);
    }

    #[test]
    fn pacify_clamps_at_zero() {
        let mut w = w();
        w.wrath = 30.0;
        w.pacify(200.0);
        assert_eq!(w.wrath, 0.0);
    }

    #[test]
    fn pacify_fires_just_calm_at_zero() {
        let mut w = w();
        w.wrath = 30.0;
        w.pacify(30.0);
        assert!(w.just_calm);
    }

    #[test]
    fn pacify_no_op_when_already_calm() {
        let mut w = w();
        w.pacify(10.0);
        assert!(!w.just_calm);
    }

    #[test]
    fn pacify_no_op_when_disabled() {
        let mut w = w();
        w.wrath = 50.0;
        w.enabled = false;
        w.pacify(50.0);
        assert!((w.wrath - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_wrath() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.wrath - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_wroth_on_wrath_to_max() {
        let mut w = Wroth::new(100.0, 200.0);
        w.wrath = 95.0;
        w.tick(1.0);
        assert!(w.just_wroth);
        assert!(w.is_wroth());
    }

    #[test]
    fn tick_no_build_when_already_wroth() {
        let mut w = w();
        w.wrath = 100.0;
        w.tick(1.0);
        assert!(!w.just_wroth);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Wroth::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.wrath, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.wrath, 0.0);
    }

    #[test]
    fn tick_clears_just_wroth() {
        let mut w = Wroth::new(100.0, 200.0);
        w.wrath = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_wroth);
    }

    #[test]
    fn tick_clears_just_calm() {
        let mut w = w();
        w.wrath = 10.0;
        w.pacify(10.0);
        w.tick(0.016);
        assert!(!w.just_calm);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.wrath - 9.0).abs() < 1e-3);
    }

    // --- is_wroth / is_calm ---

    #[test]
    fn is_wroth_false_when_disabled() {
        let mut w = w();
        w.wrath = 100.0;
        w.enabled = false;
        assert!(!w.is_wroth());
    }

    #[test]
    fn is_calm_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_calm());
    }

    // --- wrath_fraction / effective_rage ---

    #[test]
    fn wrath_fraction_zero_when_calm() {
        assert_eq!(w().wrath_fraction(), 0.0);
    }

    #[test]
    fn wrath_fraction_half_at_midpoint() {
        let mut w = w();
        w.wrath = 50.0;
        assert!((w.wrath_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_rage_zero_when_calm() {
        assert_eq!(w().effective_rage(100.0), 0.0);
    }

    #[test]
    fn effective_rage_scales_with_wrath() {
        let mut w = w();
        w.wrath = 75.0;
        assert!((w.effective_rage(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_rage_zero_when_disabled() {
        let mut w = w();
        w.wrath = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_rage(100.0), 0.0);
    }
}
