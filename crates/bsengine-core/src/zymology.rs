use bevy_ecs::prelude::Component;

/// Fermentation-science proficiency tracker. `knowledge` builds via
/// `study(amount)` and deepens passively at `research_rate` per second in
/// `tick(dt)` or is lost via `forget(amount)`.
///
/// Models fermentation-science expertise bars, enzyme-kinetics comprehension
/// gauges, brewing-process knowledge accumulators, yeast-activity proficiency
/// trackers, malt-conversion understanding fill levels, wort-chemistry mastery
/// meters, lactic-acid-bacteria taxonomy depth indicators, fermentation-vessel
/// efficiency optimization bars, or any mechanic where patient scientific
/// inquiry into the biochemistry of microbe-driven sugar-to-alcohol conversion
/// accretes into genuine mastery — until a missed protocol collapses the
/// understanding and the practitioner must rediscover the correct pH window
/// from first principles.
///
/// `study(amount)` adds knowledge; fires `just_mastered` when first
/// reaching `max_knowledge`. No-op when disabled.
///
/// `forget(amount)` reduces knowledge immediately; fires `just_confused`
/// when reaching 0. No-op when disabled or already confused.
///
/// `tick(dt)` clears both flags, then increases knowledge by
/// `research_rate * dt` (capped at `max_knowledge`). Fires `just_mastered`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_mastered()` returns `knowledge >= max_knowledge && enabled`.
///
/// `is_confused()` returns `knowledge == 0.0` (not gated by `enabled`).
///
/// `knowledge_fraction()` returns `(knowledge / max_knowledge).clamp(0, 1)`.
///
/// `effective_insight(scale)` returns `scale * knowledge_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.0)` — researches at 1 unit/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zymology {
    pub knowledge: f32,
    pub max_knowledge: f32,
    pub research_rate: f32,
    pub just_mastered: bool,
    pub just_confused: bool,
    pub enabled: bool,
}

impl Zymology {
    pub fn new(max_knowledge: f32, research_rate: f32) -> Self {
        Self {
            knowledge: 0.0,
            max_knowledge: max_knowledge.max(0.1),
            research_rate: research_rate.max(0.0),
            just_mastered: false,
            just_confused: false,
            enabled: true,
        }
    }

    /// Add knowledge; fires `just_mastered` when first reaching max.
    /// No-op when disabled.
    pub fn study(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.knowledge < self.max_knowledge;
        self.knowledge = (self.knowledge + amount).min(self.max_knowledge);
        if was_below && self.knowledge >= self.max_knowledge {
            self.just_mastered = true;
        }
    }

    /// Reduce knowledge; fires `just_confused` when reaching 0.
    /// No-op when disabled or already confused.
    pub fn forget(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.knowledge <= 0.0 {
            return;
        }
        self.knowledge = (self.knowledge - amount).max(0.0);
        if self.knowledge <= 0.0 {
            self.just_confused = true;
        }
    }

    /// Clear flags, then increase knowledge by `research_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_mastered = false;
        self.just_confused = false;
        if self.enabled && self.research_rate > 0.0 && self.knowledge < self.max_knowledge {
            let was_below = self.knowledge < self.max_knowledge;
            self.knowledge = (self.knowledge + self.research_rate * dt).min(self.max_knowledge);
            if was_below && self.knowledge >= self.max_knowledge {
                self.just_mastered = true;
            }
        }
    }

    /// `true` when knowledge is at maximum and component is enabled.
    pub fn is_mastered(&self) -> bool {
        self.knowledge >= self.max_knowledge && self.enabled
    }

    /// `true` when knowledge is 0 (not gated by `enabled`).
    pub fn is_confused(&self) -> bool {
        self.knowledge == 0.0
    }

    /// Fraction of maximum knowledge [0.0, 1.0].
    pub fn knowledge_fraction(&self) -> f32 {
        (self.knowledge / self.max_knowledge).clamp(0.0, 1.0)
    }

    /// Returns `scale * knowledge_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_insight(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.knowledge_fraction()
    }
}

impl Default for Zymology {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zymology {
        Zymology::new(100.0, 1.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_confused() {
        let z = z();
        assert_eq!(z.knowledge, 0.0);
        assert!(z.is_confused());
        assert!(!z.is_mastered());
    }

    #[test]
    fn new_clamps_max_knowledge() {
        let z = Zymology::new(-5.0, 1.0);
        assert!((z.max_knowledge - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_research_rate() {
        let z = Zymology::new(100.0, -1.0);
        assert_eq!(z.research_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zymology::default();
        assert!((z.max_knowledge - 100.0).abs() < 1e-5);
        assert!((z.research_rate - 1.0).abs() < 1e-5);
    }

    // --- study ---

    #[test]
    fn study_adds_knowledge() {
        let mut z = z();
        z.study(40.0);
        assert!((z.knowledge - 40.0).abs() < 1e-3);
    }

    #[test]
    fn study_clamps_at_max() {
        let mut z = z();
        z.study(200.0);
        assert!((z.knowledge - 100.0).abs() < 1e-3);
    }

    #[test]
    fn study_fires_just_mastered_at_max() {
        let mut z = z();
        z.study(100.0);
        assert!(z.just_mastered);
        assert!(z.is_mastered());
    }

    #[test]
    fn study_no_just_mastered_when_already_at_max() {
        let mut z = z();
        z.knowledge = 100.0;
        z.study(10.0);
        assert!(!z.just_mastered);
    }

    #[test]
    fn study_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.study(50.0);
        assert_eq!(z.knowledge, 0.0);
    }

    #[test]
    fn study_no_op_when_amount_zero() {
        let mut z = z();
        z.study(0.0);
        assert_eq!(z.knowledge, 0.0);
    }

    // --- forget ---

    #[test]
    fn forget_reduces_knowledge() {
        let mut z = z();
        z.knowledge = 60.0;
        z.forget(20.0);
        assert!((z.knowledge - 40.0).abs() < 1e-3);
    }

    #[test]
    fn forget_clamps_at_zero() {
        let mut z = z();
        z.knowledge = 30.0;
        z.forget(200.0);
        assert_eq!(z.knowledge, 0.0);
    }

    #[test]
    fn forget_fires_just_confused_at_zero() {
        let mut z = z();
        z.knowledge = 30.0;
        z.forget(30.0);
        assert!(z.just_confused);
    }

    #[test]
    fn forget_no_op_when_already_confused() {
        let mut z = z();
        z.forget(10.0);
        assert!(!z.just_confused);
    }

    #[test]
    fn forget_no_op_when_disabled() {
        let mut z = z();
        z.knowledge = 50.0;
        z.enabled = false;
        z.forget(50.0);
        assert!((z.knowledge - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_researches_knowledge() {
        let mut z = z(); // rate=1
        z.tick(7.0); // 0 + 1*7 = 7
        assert!((z.knowledge - 7.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_mastered_on_research_to_max() {
        let mut z = Zymology::new(100.0, 200.0);
        z.knowledge = 95.0;
        z.tick(1.0);
        assert!(z.just_mastered);
        assert!(z.is_mastered());
    }

    #[test]
    fn tick_no_research_when_already_mastered() {
        let mut z = z();
        z.knowledge = 100.0;
        z.tick(1.0);
        assert!(!z.just_mastered);
    }

    #[test]
    fn tick_no_research_when_rate_zero() {
        let mut z = Zymology::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.knowledge, 0.0);
    }

    #[test]
    fn tick_no_research_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.knowledge, 0.0);
    }

    #[test]
    fn tick_clears_just_mastered() {
        let mut z = Zymology::new(100.0, 200.0);
        z.knowledge = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_mastered);
    }

    #[test]
    fn tick_clears_just_confused() {
        let mut z = z();
        z.knowledge = 10.0;
        z.forget(10.0);
        z.tick(0.016);
        assert!(!z.just_confused);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1
        z.tick(9.0); // 1*9 = 9
        assert!((z.knowledge - 9.0).abs() < 1e-3);
    }

    // --- is_mastered / is_confused ---

    #[test]
    fn is_mastered_false_when_disabled() {
        let mut z = z();
        z.knowledge = 100.0;
        z.enabled = false;
        assert!(!z.is_mastered());
    }

    #[test]
    fn is_confused_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_confused());
    }

    // --- knowledge_fraction / effective_insight ---

    #[test]
    fn knowledge_fraction_zero_when_confused() {
        assert_eq!(z().knowledge_fraction(), 0.0);
    }

    #[test]
    fn knowledge_fraction_half_at_midpoint() {
        let mut z = z();
        z.knowledge = 50.0;
        assert!((z.knowledge_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_insight_zero_when_confused() {
        assert_eq!(z().effective_insight(100.0), 0.0);
    }

    #[test]
    fn effective_insight_scales_with_knowledge() {
        let mut z = z();
        z.knowledge = 75.0;
        assert!((z.effective_insight(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_insight_zero_when_disabled() {
        let mut z = z();
        z.knowledge = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_insight(100.0), 0.0);
    }
}
