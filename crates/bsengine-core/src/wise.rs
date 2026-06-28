use bevy_ecs::prelude::Component;

/// Wisdom-discernment accumulation tracker named after wise,
/// the adjective meaning having or showing experience, knowledge,
/// and good judgment; prudent and sensible; having the power
/// of discerning and judging properly as to what is true or
/// right — from the Old English wīs (learned, wise, prudent,
/// discreet, experienced), from the Proto-Germanic wīsaz
/// (knowing, wise), from the Proto-Indo-European root weid-
/// (to see, to know). The root weid- also gave vision, video,
/// idea, wit, guide, and witness — all words about knowing
/// through seeing or having seen. To be wise is, at its root,
/// to have seen enough to understand: the wise person is the
/// one who has witnessed enough of the world's workings to
/// perceive patterns that escape those with less experience.
/// Wisdom, in the ancient taxonomies, was distinguished from
/// mere cleverness (knowing how to do things) and from
/// knowledge (knowing that things are so) by its evaluative
/// and practical dimension: the wise person knows what
/// matters, what to do, and when. In spiritual traditions,
/// wisdom is often figured as a slow accumulation — it cannot
/// be hurried, cannot be transmitted directly, can only be
/// earned through attention and time. In game mechanics, a
/// wise mechanic models the accumulation of experiential
/// understanding — the build of insight, discernment, or
/// learned judgment that eventually reaches the threshold at
/// which a character perceives what others miss, resists
/// manipulation, or unlocks wisdom-dependent abilities.
/// `insight` builds via `observe(amount)` and accumulates
/// passively at `discern_rate` per second in `tick(dt)` or
/// is clouded via `confound(amount)`.
///
/// Models wisdom-discernment fill levels, insight-saturation
/// bars, judgment-accumulation trackers, prudence-build gauges,
/// sage fill levels, enlightenment-saturation indicators,
/// understanding-accumulation bars, discernment meters,
/// clarity-completion fill levels, or any mechanic where a
/// character slowly accumulates the insights, experiences,
/// or learned judgments that grant them the ability to see
/// through deception, make sound decisions under pressure,
/// or access wisdom-dependent knowledge or abilities.
///
/// `observe(amount)` adds insight; fires `just_wise` when
/// first reaching `max_insight`. No-op when disabled.
///
/// `confound(amount)` reduces insight immediately; fires
/// `just_clouded` when reaching 0. No-op when disabled or
/// already clouded.
///
/// `tick(dt)` clears both flags, then increases insight by
/// `discern_rate * dt` (capped at `max_insight`). Fires
/// `just_wise` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_wise()` returns `insight >= max_insight && enabled`.
///
/// `is_clouded()` returns `insight == 0.0` (not gated by
/// `enabled`).
///
/// `insight_fraction()` returns
/// `(insight / max_insight).clamp(0, 1)`.
///
/// `effective_judgment(scale)` returns
/// `scale * insight_fraction()` when enabled; `0.0` when
/// disabled.
///
/// Default: `new(100.0, 1.5)` — discerns at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wise {
    pub insight: f32,
    pub max_insight: f32,
    pub discern_rate: f32,
    pub just_wise: bool,
    pub just_clouded: bool,
    pub enabled: bool,
}

impl Wise {
    pub fn new(max_insight: f32, discern_rate: f32) -> Self {
        Self {
            insight: 0.0,
            max_insight: max_insight.max(0.1),
            discern_rate: discern_rate.max(0.0),
            just_wise: false,
            just_clouded: false,
            enabled: true,
        }
    }

    /// Add insight; fires `just_wise` when first reaching max.
    /// No-op when disabled.
    pub fn observe(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.insight < self.max_insight;
        self.insight = (self.insight + amount).min(self.max_insight);
        if was_below && self.insight >= self.max_insight {
            self.just_wise = true;
        }
    }

    /// Reduce insight; fires `just_clouded` when reaching 0.
    /// No-op when disabled or already clouded.
    pub fn confound(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.insight <= 0.0 {
            return;
        }
        self.insight = (self.insight - amount).max(0.0);
        if self.insight <= 0.0 {
            self.just_clouded = true;
        }
    }

    /// Clear flags, then increase insight by `discern_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_wise = false;
        self.just_clouded = false;
        if self.enabled && self.discern_rate > 0.0 && self.insight < self.max_insight {
            let was_below = self.insight < self.max_insight;
            self.insight = (self.insight + self.discern_rate * dt).min(self.max_insight);
            if was_below && self.insight >= self.max_insight {
                self.just_wise = true;
            }
        }
    }

    /// `true` when insight is at maximum and component is enabled.
    pub fn is_wise(&self) -> bool {
        self.insight >= self.max_insight && self.enabled
    }

    /// `true` when insight is 0 (not gated by `enabled`).
    pub fn is_clouded(&self) -> bool {
        self.insight == 0.0
    }

    /// Fraction of maximum insight [0.0, 1.0].
    pub fn insight_fraction(&self) -> f32 {
        (self.insight / self.max_insight).clamp(0.0, 1.0)
    }

    /// Returns `scale * insight_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_judgment(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.insight_fraction()
    }
}

impl Default for Wise {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wise {
        Wise::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_clouded() {
        let w = w();
        assert_eq!(w.insight, 0.0);
        assert!(w.is_clouded());
        assert!(!w.is_wise());
    }

    #[test]
    fn new_clamps_max_insight() {
        let w = Wise::new(-5.0, 1.5);
        assert!((w.max_insight - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_discern_rate() {
        let w = Wise::new(100.0, -1.5);
        assert_eq!(w.discern_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Wise::default();
        assert!((w.max_insight - 100.0).abs() < 1e-5);
        assert!((w.discern_rate - 1.5).abs() < 1e-5);
    }

    // --- observe ---

    #[test]
    fn observe_adds_insight() {
        let mut w = w();
        w.observe(40.0);
        assert!((w.insight - 40.0).abs() < 1e-3);
    }

    #[test]
    fn observe_clamps_at_max() {
        let mut w = w();
        w.observe(200.0);
        assert!((w.insight - 100.0).abs() < 1e-3);
    }

    #[test]
    fn observe_fires_just_wise_at_max() {
        let mut w = w();
        w.observe(100.0);
        assert!(w.just_wise);
        assert!(w.is_wise());
    }

    #[test]
    fn observe_no_just_wise_when_already_at_max() {
        let mut w = w();
        w.insight = 100.0;
        w.observe(10.0);
        assert!(!w.just_wise);
    }

    #[test]
    fn observe_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.observe(50.0);
        assert_eq!(w.insight, 0.0);
    }

    #[test]
    fn observe_no_op_when_amount_zero() {
        let mut w = w();
        w.observe(0.0);
        assert_eq!(w.insight, 0.0);
    }

    // --- confound ---

    #[test]
    fn confound_reduces_insight() {
        let mut w = w();
        w.insight = 60.0;
        w.confound(20.0);
        assert!((w.insight - 40.0).abs() < 1e-3);
    }

    #[test]
    fn confound_clamps_at_zero() {
        let mut w = w();
        w.insight = 30.0;
        w.confound(200.0);
        assert_eq!(w.insight, 0.0);
    }

    #[test]
    fn confound_fires_just_clouded_at_zero() {
        let mut w = w();
        w.insight = 30.0;
        w.confound(30.0);
        assert!(w.just_clouded);
    }

    #[test]
    fn confound_no_op_when_already_clouded() {
        let mut w = w();
        w.confound(10.0);
        assert!(!w.just_clouded);
    }

    #[test]
    fn confound_no_op_when_disabled() {
        let mut w = w();
        w.insight = 50.0;
        w.enabled = false;
        w.confound(50.0);
        assert!((w.insight - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_insight() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.insight - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_wise_on_insight_to_max() {
        let mut w = Wise::new(100.0, 200.0);
        w.insight = 95.0;
        w.tick(1.0);
        assert!(w.just_wise);
        assert!(w.is_wise());
    }

    #[test]
    fn tick_no_build_when_already_wise() {
        let mut w = w();
        w.insight = 100.0;
        w.tick(1.0);
        assert!(!w.just_wise);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Wise::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.insight, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.insight, 0.0);
    }

    #[test]
    fn tick_clears_just_wise() {
        let mut w = Wise::new(100.0, 200.0);
        w.insight = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_wise);
    }

    #[test]
    fn tick_clears_just_clouded() {
        let mut w = w();
        w.insight = 10.0;
        w.confound(10.0);
        w.tick(0.016);
        assert!(!w.just_clouded);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.insight - 9.0).abs() < 1e-3);
    }

    // --- is_wise / is_clouded ---

    #[test]
    fn is_wise_false_when_disabled() {
        let mut w = w();
        w.insight = 100.0;
        w.enabled = false;
        assert!(!w.is_wise());
    }

    #[test]
    fn is_clouded_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_clouded());
    }

    // --- insight_fraction / effective_judgment ---

    #[test]
    fn insight_fraction_zero_when_clouded() {
        assert_eq!(w().insight_fraction(), 0.0);
    }

    #[test]
    fn insight_fraction_half_at_midpoint() {
        let mut w = w();
        w.insight = 50.0;
        assert!((w.insight_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_judgment_zero_when_clouded() {
        assert_eq!(w().effective_judgment(100.0), 0.0);
    }

    #[test]
    fn effective_judgment_scales_with_insight() {
        let mut w = w();
        w.insight = 75.0;
        assert!((w.effective_judgment(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_judgment_zero_when_disabled() {
        let mut w = w();
        w.insight = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_judgment(100.0), 0.0);
    }
}
