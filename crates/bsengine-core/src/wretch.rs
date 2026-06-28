use bevy_ecs::prelude::Component;

/// Misery-suffering accumulation tracker named after wretch,
/// the noun meaning a miserable or unhappy person; one who
/// is in a state of deep affliction or distress; a base,
/// despicable, or contemptible person — from the Old English
/// wrecca (an exile, an outcast, a miserable person, an
/// adventurer), from the Proto-Germanic wragjaz (one who
/// has been driven out), related to the Old English wrecan
/// (to drive out, to punish, to avenge) and to the modern
/// English wreak (to inflict, to visit upon). The wretch,
/// originally, was the driven-out one — the person expelled
/// from their community, their home, their protection. Exile
/// was among the worst fates available in early medieval
/// society, and the word wretch carries that weight: it is
/// not merely sadness but the condition of the outcast,
/// the unprotected, the one who has lost all the social
/// relationships that make human life bearable. Over time
/// the word generalized from the specific condition of
/// exile to any condition of profound misery, and then to
/// moral condemnation — a wretch can be pitiable or
/// contemptible, depending on context; the wretched poor
/// are objects of compassion, the wretched traitor is an
/// object of contempt. In game mechanics, a wretch mechanic
/// models the accumulation of misery, suffering, or
/// affliction — the build of despair, social deprivation,
/// or existential wretchedness that eventually reaches the
/// threshold at which a character enters a state of profound
/// dysfunction, is consumed by despair, or triggers a
/// misery-dependent event. `misery` builds via
/// `despair(amount)` and accumulates passively at
/// `suffer_rate` per second in `tick(dt)` or is consoled
/// via `console(amount)`.
///
/// Models misery-suffering fill levels, despair-saturation
/// bars, affliction-accumulation trackers, wretchedness-build
/// gauges, sorrow fill levels, anguish-saturation indicators,
/// distress-accumulation bars, affliction meters, misery-
/// completion fill levels, or any mechanic where a character
/// slowly accumulates the suffering, deprivation, or social
/// wretchedness that leads to dysfunction, despair, or
/// a triggered consequence of deep misery.
///
/// `despair(amount)` adds misery; fires `just_wretched` when
/// first reaching `max_misery`. No-op when disabled.
///
/// `console(amount)` reduces misery immediately; fires
/// `just_relieved` when reaching 0. No-op when disabled or
/// already relieved.
///
/// `tick(dt)` clears both flags, then increases misery by
/// `suffer_rate * dt` (capped at `max_misery`). Fires
/// `just_wretched` when first reaching max. No-op when
/// disabled or rate is 0.
///
/// `is_wretched()` returns `misery >= max_misery && enabled`.
///
/// `is_relieved()` returns `misery == 0.0` (not gated by
/// `enabled`).
///
/// `misery_fraction()` returns
/// `(misery / max_misery).clamp(0, 1)`.
///
/// `effective_suffering(scale)` returns
/// `scale * misery_fraction()` when enabled; `0.0` when
/// disabled.
///
/// Default: `new(100.0, 1.5)` — suffers at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wretch {
    pub misery: f32,
    pub max_misery: f32,
    pub suffer_rate: f32,
    pub just_wretched: bool,
    pub just_relieved: bool,
    pub enabled: bool,
}

impl Wretch {
    pub fn new(max_misery: f32, suffer_rate: f32) -> Self {
        Self {
            misery: 0.0,
            max_misery: max_misery.max(0.1),
            suffer_rate: suffer_rate.max(0.0),
            just_wretched: false,
            just_relieved: false,
            enabled: true,
        }
    }

    /// Add misery; fires `just_wretched` when first reaching max.
    /// No-op when disabled.
    pub fn despair(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.misery < self.max_misery;
        self.misery = (self.misery + amount).min(self.max_misery);
        if was_below && self.misery >= self.max_misery {
            self.just_wretched = true;
        }
    }

    /// Reduce misery; fires `just_relieved` when reaching 0.
    /// No-op when disabled or already relieved.
    pub fn console(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.misery <= 0.0 {
            return;
        }
        self.misery = (self.misery - amount).max(0.0);
        if self.misery <= 0.0 {
            self.just_relieved = true;
        }
    }

    /// Clear flags, then increase misery by `suffer_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_wretched = false;
        self.just_relieved = false;
        if self.enabled && self.suffer_rate > 0.0 && self.misery < self.max_misery {
            let was_below = self.misery < self.max_misery;
            self.misery = (self.misery + self.suffer_rate * dt).min(self.max_misery);
            if was_below && self.misery >= self.max_misery {
                self.just_wretched = true;
            }
        }
    }

    /// `true` when misery is at maximum and component is enabled.
    pub fn is_wretched(&self) -> bool {
        self.misery >= self.max_misery && self.enabled
    }

    /// `true` when misery is 0 (not gated by `enabled`).
    pub fn is_relieved(&self) -> bool {
        self.misery == 0.0
    }

    /// Fraction of maximum misery [0.0, 1.0].
    pub fn misery_fraction(&self) -> f32 {
        (self.misery / self.max_misery).clamp(0.0, 1.0)
    }

    /// Returns `scale * misery_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_suffering(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.misery_fraction()
    }
}

impl Default for Wretch {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wretch {
        Wretch::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_relieved() {
        let w = w();
        assert_eq!(w.misery, 0.0);
        assert!(w.is_relieved());
        assert!(!w.is_wretched());
    }

    #[test]
    fn new_clamps_max_misery() {
        let w = Wretch::new(-5.0, 1.5);
        assert!((w.max_misery - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_suffer_rate() {
        let w = Wretch::new(100.0, -1.5);
        assert_eq!(w.suffer_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Wretch::default();
        assert!((w.max_misery - 100.0).abs() < 1e-5);
        assert!((w.suffer_rate - 1.5).abs() < 1e-5);
    }

    // --- despair ---

    #[test]
    fn despair_adds_misery() {
        let mut w = w();
        w.despair(40.0);
        assert!((w.misery - 40.0).abs() < 1e-3);
    }

    #[test]
    fn despair_clamps_at_max() {
        let mut w = w();
        w.despair(200.0);
        assert!((w.misery - 100.0).abs() < 1e-3);
    }

    #[test]
    fn despair_fires_just_wretched_at_max() {
        let mut w = w();
        w.despair(100.0);
        assert!(w.just_wretched);
        assert!(w.is_wretched());
    }

    #[test]
    fn despair_no_just_wretched_when_already_at_max() {
        let mut w = w();
        w.misery = 100.0;
        w.despair(10.0);
        assert!(!w.just_wretched);
    }

    #[test]
    fn despair_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.despair(50.0);
        assert_eq!(w.misery, 0.0);
    }

    #[test]
    fn despair_no_op_when_amount_zero() {
        let mut w = w();
        w.despair(0.0);
        assert_eq!(w.misery, 0.0);
    }

    // --- console ---

    #[test]
    fn console_reduces_misery() {
        let mut w = w();
        w.misery = 60.0;
        w.console(20.0);
        assert!((w.misery - 40.0).abs() < 1e-3);
    }

    #[test]
    fn console_clamps_at_zero() {
        let mut w = w();
        w.misery = 30.0;
        w.console(200.0);
        assert_eq!(w.misery, 0.0);
    }

    #[test]
    fn console_fires_just_relieved_at_zero() {
        let mut w = w();
        w.misery = 30.0;
        w.console(30.0);
        assert!(w.just_relieved);
    }

    #[test]
    fn console_no_op_when_already_relieved() {
        let mut w = w();
        w.console(10.0);
        assert!(!w.just_relieved);
    }

    #[test]
    fn console_no_op_when_disabled() {
        let mut w = w();
        w.misery = 50.0;
        w.enabled = false;
        w.console(50.0);
        assert!((w.misery - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_misery() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.misery - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_wretched_on_misery_to_max() {
        let mut w = Wretch::new(100.0, 200.0);
        w.misery = 95.0;
        w.tick(1.0);
        assert!(w.just_wretched);
        assert!(w.is_wretched());
    }

    #[test]
    fn tick_no_build_when_already_wretched() {
        let mut w = w();
        w.misery = 100.0;
        w.tick(1.0);
        assert!(!w.just_wretched);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Wretch::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.misery, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.misery, 0.0);
    }

    #[test]
    fn tick_clears_just_wretched() {
        let mut w = Wretch::new(100.0, 200.0);
        w.misery = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_wretched);
    }

    #[test]
    fn tick_clears_just_relieved() {
        let mut w = w();
        w.misery = 10.0;
        w.console(10.0);
        w.tick(0.016);
        assert!(!w.just_relieved);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.misery - 9.0).abs() < 1e-3);
    }

    // --- is_wretched / is_relieved ---

    #[test]
    fn is_wretched_false_when_disabled() {
        let mut w = w();
        w.misery = 100.0;
        w.enabled = false;
        assert!(!w.is_wretched());
    }

    #[test]
    fn is_relieved_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_relieved());
    }

    // --- misery_fraction / effective_suffering ---

    #[test]
    fn misery_fraction_zero_when_relieved() {
        assert_eq!(w().misery_fraction(), 0.0);
    }

    #[test]
    fn misery_fraction_half_at_midpoint() {
        let mut w = w();
        w.misery = 50.0;
        assert!((w.misery_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_suffering_zero_when_relieved() {
        assert_eq!(w().effective_suffering(100.0), 0.0);
    }

    #[test]
    fn effective_suffering_scales_with_misery() {
        let mut w = w();
        w.misery = 75.0;
        assert!((w.effective_suffering(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_suffering_zero_when_disabled() {
        let mut w = w();
        w.misery = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_suffering(100.0), 0.0);
    }
}
