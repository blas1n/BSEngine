use bevy_ecs::prelude::Component;

/// Zoological-expertise accumulation tracker named after the zoologist,
/// the specialist whose career is devoted to the systematic study of
/// animal life — its taxonomy, physiology, behaviour, ecology, and
/// evolutionary history. The discipline traces its modern form to the
/// nineteenth century: John Ray establishing binomial principles that
/// Linnaeus would formalise; Georges Cuvier founding comparative anatomy
/// and using it to prove that fossil species had gone extinct; Charles
/// Darwin and Alfred Russel Wallace independently arriving at natural
/// selection by compiling field observations across continents and
/// oceans. The zoologist's expertise accumulates through the same
/// process regardless of era: patient observation logged in field
/// notebooks, specimens pinned and measured, literature read and
/// integrated, mentors questioned and peers debated, until the expert's
/// eye can read the age of a bird from the colour of its bare parts
/// or place an unfamiliar insect in its correct family from the
/// venation pattern of a single wing. That expertise is not permanent
/// without maintenance: a specialist who abandons a taxon for a decade
/// returns to find new genera erected, old synonymies overturned,
/// molecular phylogenies displacing morphological ones, and the
/// confident knowledge that once felt encyclopaedic now riddled with
/// gaps. `expertise` builds via `observe(amount)` and accumulates
/// passively at `study_rate` per second in `tick(dt)` or erodes via
/// `lapse(amount)`.
///
/// Models zoological-expertise fill levels, naturalist-proficiency
/// saturation bars, field-identification-skill accumulators, taxonomic-
/// mastery gauges, species-knowledge saturation indicators, fieldwork-
/// competence fill levels, comparative-anatomy-skill bars, wildlife-
/// survey-expertise accumulators, expedition-readiness meters, or any
/// mechanic where systematic observation slowly builds a mental atlas
/// of animal life until the protagonist can navigate any ecosystem like
/// a map they drew themselves — and where inactivity or trauma erodes
/// that hard-won knowledge back toward the uncertainty of a first field
/// season.
///
/// `observe(amount)` adds expertise; fires `just_mastered` when first
/// reaching `max_expertise`. No-op when disabled.
///
/// `lapse(amount)` reduces expertise immediately; fires `just_lapsed`
/// when reaching 0. No-op when disabled or already lapsed.
///
/// `tick(dt)` clears both flags, then increases expertise by
/// `study_rate * dt` (capped at `max_expertise`). Fires `just_mastered`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_mastered()` returns `expertise >= max_expertise && enabled`.
///
/// `is_lapsed()` returns `expertise == 0.0` (not gated by `enabled`).
///
/// `expertise_fraction()` returns
/// `(expertise / max_expertise).clamp(0, 1)`.
///
/// `effective_scholarship(scale)` returns `scale * expertise_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — studies at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoologist {
    pub expertise: f32,
    pub max_expertise: f32,
    pub study_rate: f32,
    pub just_mastered: bool,
    pub just_lapsed: bool,
    pub enabled: bool,
}

impl Zoologist {
    pub fn new(max_expertise: f32, study_rate: f32) -> Self {
        Self {
            expertise: 0.0,
            max_expertise: max_expertise.max(0.1),
            study_rate: study_rate.max(0.0),
            just_mastered: false,
            just_lapsed: false,
            enabled: true,
        }
    }

    /// Add expertise; fires `just_mastered` when first reaching max.
    /// No-op when disabled.
    pub fn observe(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.expertise < self.max_expertise;
        self.expertise = (self.expertise + amount).min(self.max_expertise);
        if was_below && self.expertise >= self.max_expertise {
            self.just_mastered = true;
        }
    }

    /// Reduce expertise; fires `just_lapsed` when reaching 0.
    /// No-op when disabled or already lapsed.
    pub fn lapse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.expertise <= 0.0 {
            return;
        }
        self.expertise = (self.expertise - amount).max(0.0);
        if self.expertise <= 0.0 {
            self.just_lapsed = true;
        }
    }

    /// Clear flags, then increase expertise by `study_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_mastered = false;
        self.just_lapsed = false;
        if self.enabled && self.study_rate > 0.0 && self.expertise < self.max_expertise {
            let was_below = self.expertise < self.max_expertise;
            self.expertise = (self.expertise + self.study_rate * dt).min(self.max_expertise);
            if was_below && self.expertise >= self.max_expertise {
                self.just_mastered = true;
            }
        }
    }

    /// `true` when expertise is at maximum and component is enabled.
    pub fn is_mastered(&self) -> bool {
        self.expertise >= self.max_expertise && self.enabled
    }

    /// `true` when expertise is 0 (not gated by `enabled`).
    pub fn is_lapsed(&self) -> bool {
        self.expertise == 0.0
    }

    /// Fraction of maximum expertise [0.0, 1.0].
    pub fn expertise_fraction(&self) -> f32 {
        (self.expertise / self.max_expertise).clamp(0.0, 1.0)
    }

    /// Returns `scale * expertise_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_scholarship(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.expertise_fraction()
    }
}

impl Default for Zoologist {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoologist {
        Zoologist::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_lapsed() {
        let z = z();
        assert_eq!(z.expertise, 0.0);
        assert!(z.is_lapsed());
        assert!(!z.is_mastered());
    }

    #[test]
    fn new_clamps_max_expertise() {
        let z = Zoologist::new(-5.0, 1.5);
        assert!((z.max_expertise - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_study_rate() {
        let z = Zoologist::new(100.0, -1.5);
        assert_eq!(z.study_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoologist::default();
        assert!((z.max_expertise - 100.0).abs() < 1e-5);
        assert!((z.study_rate - 1.5).abs() < 1e-5);
    }

    // --- observe ---

    #[test]
    fn observe_adds_expertise() {
        let mut z = z();
        z.observe(40.0);
        assert!((z.expertise - 40.0).abs() < 1e-3);
    }

    #[test]
    fn observe_clamps_at_max() {
        let mut z = z();
        z.observe(200.0);
        assert!((z.expertise - 100.0).abs() < 1e-3);
    }

    #[test]
    fn observe_fires_just_mastered_at_max() {
        let mut z = z();
        z.observe(100.0);
        assert!(z.just_mastered);
        assert!(z.is_mastered());
    }

    #[test]
    fn observe_no_just_mastered_when_already_at_max() {
        let mut z = z();
        z.expertise = 100.0;
        z.observe(10.0);
        assert!(!z.just_mastered);
    }

    #[test]
    fn observe_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.observe(50.0);
        assert_eq!(z.expertise, 0.0);
    }

    #[test]
    fn observe_no_op_when_amount_zero() {
        let mut z = z();
        z.observe(0.0);
        assert_eq!(z.expertise, 0.0);
    }

    // --- lapse ---

    #[test]
    fn lapse_reduces_expertise() {
        let mut z = z();
        z.expertise = 60.0;
        z.lapse(20.0);
        assert!((z.expertise - 40.0).abs() < 1e-3);
    }

    #[test]
    fn lapse_clamps_at_zero() {
        let mut z = z();
        z.expertise = 30.0;
        z.lapse(200.0);
        assert_eq!(z.expertise, 0.0);
    }

    #[test]
    fn lapse_fires_just_lapsed_at_zero() {
        let mut z = z();
        z.expertise = 30.0;
        z.lapse(30.0);
        assert!(z.just_lapsed);
    }

    #[test]
    fn lapse_no_op_when_already_lapsed() {
        let mut z = z();
        z.lapse(10.0);
        assert!(!z.just_lapsed);
    }

    #[test]
    fn lapse_no_op_when_disabled() {
        let mut z = z();
        z.expertise = 50.0;
        z.enabled = false;
        z.lapse(50.0);
        assert!((z.expertise - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_studies_expertise() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.expertise - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_mastered_on_study_to_max() {
        let mut z = Zoologist::new(100.0, 200.0);
        z.expertise = 95.0;
        z.tick(1.0);
        assert!(z.just_mastered);
        assert!(z.is_mastered());
    }

    #[test]
    fn tick_no_study_when_already_mastered() {
        let mut z = z();
        z.expertise = 100.0;
        z.tick(1.0);
        assert!(!z.just_mastered);
    }

    #[test]
    fn tick_no_study_when_rate_zero() {
        let mut z = Zoologist::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.expertise, 0.0);
    }

    #[test]
    fn tick_no_study_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.expertise, 0.0);
    }

    #[test]
    fn tick_clears_just_mastered() {
        let mut z = Zoologist::new(100.0, 200.0);
        z.expertise = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_mastered);
    }

    #[test]
    fn tick_clears_just_lapsed() {
        let mut z = z();
        z.expertise = 10.0;
        z.lapse(10.0);
        z.tick(0.016);
        assert!(!z.just_lapsed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.expertise - 9.0).abs() < 1e-3);
    }

    // --- is_mastered / is_lapsed ---

    #[test]
    fn is_mastered_false_when_disabled() {
        let mut z = z();
        z.expertise = 100.0;
        z.enabled = false;
        assert!(!z.is_mastered());
    }

    #[test]
    fn is_lapsed_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_lapsed());
    }

    // --- expertise_fraction / effective_scholarship ---

    #[test]
    fn expertise_fraction_zero_when_lapsed() {
        assert_eq!(z().expertise_fraction(), 0.0);
    }

    #[test]
    fn expertise_fraction_half_at_midpoint() {
        let mut z = z();
        z.expertise = 50.0;
        assert!((z.expertise_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_scholarship_zero_when_lapsed() {
        assert_eq!(z().effective_scholarship(100.0), 0.0);
    }

    #[test]
    fn effective_scholarship_scales_with_expertise() {
        let mut z = z();
        z.expertise = 75.0;
        assert!((z.effective_scholarship(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_scholarship_zero_when_disabled() {
        let mut z = z();
        z.expertise = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_scholarship(100.0), 0.0);
    }
}
