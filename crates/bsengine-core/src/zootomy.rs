use bevy_ecs::prelude::Component;

/// Specimen-dissection progress tracker. `dissection` builds via
/// `incise(amount)` and advances passively at `study_rate` per second
/// in `tick(dt)` or is contaminated immediately via `contaminate(amount)`.
///
/// Models xenobiology-specimen analysis fill levels, animal-anatomy
/// study progress bars, surgical-dissection completeness gauges,
/// comparative-anatomy specimen-coverage trackers, natural-history
/// specimen-examination intensity meters, natural-philosophy study
/// bars, veterinary-autopsy progress indicators, creature-biology
/// knowledge accumulation fill levels, field-biology specimen-
/// dissection completion trackers, or any mechanic where a
/// meticulous scholar methodically opens, describes, and records
/// every layer of a specimen until the whole organism has been
/// mapped and documented — only for a spoilage event to render
/// the sample unusable, returning the study to square one.
///
/// `incise(amount)` adds dissection; fires `just_complete` when
/// first reaching `max_dissection`. No-op when disabled.
///
/// `contaminate(amount)` reduces dissection immediately; fires
/// `just_spoiled` when reaching 0. No-op when disabled or already
/// spoiled.
///
/// `tick(dt)` clears both flags, then increases dissection by
/// `study_rate * dt` (capped at `max_dissection`). Fires
/// `just_complete` when first reaching max. No-op when disabled or
/// rate is 0.
///
/// `is_complete()` returns `dissection >= max_dissection && enabled`.
///
/// `is_spoiled()` returns `dissection == 0.0` (not gated by
/// `enabled`).
///
/// `dissection_fraction()` returns `(dissection / max_dissection).clamp(0, 1)`.
///
/// `effective_understanding(scale)` returns `scale * dissection_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.0)` — studies at 1 unit/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zootomy {
    pub dissection: f32,
    pub max_dissection: f32,
    pub study_rate: f32,
    pub just_complete: bool,
    pub just_spoiled: bool,
    pub enabled: bool,
}

impl Zootomy {
    pub fn new(max_dissection: f32, study_rate: f32) -> Self {
        Self {
            dissection: 0.0,
            max_dissection: max_dissection.max(0.1),
            study_rate: study_rate.max(0.0),
            just_complete: false,
            just_spoiled: false,
            enabled: true,
        }
    }

    /// Add dissection; fires `just_complete` when first reaching max.
    /// No-op when disabled.
    pub fn incise(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.dissection < self.max_dissection;
        self.dissection = (self.dissection + amount).min(self.max_dissection);
        if was_below && self.dissection >= self.max_dissection {
            self.just_complete = true;
        }
    }

    /// Reduce dissection; fires `just_spoiled` when reaching 0.
    /// No-op when disabled or already spoiled.
    pub fn contaminate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.dissection <= 0.0 {
            return;
        }
        self.dissection = (self.dissection - amount).max(0.0);
        if self.dissection <= 0.0 {
            self.just_spoiled = true;
        }
    }

    /// Clear flags, then increase dissection by `study_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_complete = false;
        self.just_spoiled = false;
        if self.enabled && self.study_rate > 0.0 && self.dissection < self.max_dissection {
            let was_below = self.dissection < self.max_dissection;
            self.dissection = (self.dissection + self.study_rate * dt).min(self.max_dissection);
            if was_below && self.dissection >= self.max_dissection {
                self.just_complete = true;
            }
        }
    }

    /// `true` when dissection is at maximum and component is enabled.
    pub fn is_complete(&self) -> bool {
        self.dissection >= self.max_dissection && self.enabled
    }

    /// `true` when dissection is 0 (not gated by `enabled`).
    pub fn is_spoiled(&self) -> bool {
        self.dissection == 0.0
    }

    /// Fraction of maximum dissection [0.0, 1.0].
    pub fn dissection_fraction(&self) -> f32 {
        (self.dissection / self.max_dissection).clamp(0.0, 1.0)
    }

    /// Returns `scale * dissection_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_understanding(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.dissection_fraction()
    }
}

impl Default for Zootomy {
    fn default() -> Self {
        Self::new(100.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zootomy {
        Zootomy::new(100.0, 1.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_spoiled() {
        let z = z();
        assert_eq!(z.dissection, 0.0);
        assert!(z.is_spoiled());
        assert!(!z.is_complete());
    }

    #[test]
    fn new_clamps_max_dissection() {
        let z = Zootomy::new(-5.0, 1.0);
        assert!((z.max_dissection - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_study_rate() {
        let z = Zootomy::new(100.0, -1.0);
        assert_eq!(z.study_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zootomy::default();
        assert!((z.max_dissection - 100.0).abs() < 1e-5);
        assert!((z.study_rate - 1.0).abs() < 1e-5);
    }

    // --- incise ---

    #[test]
    fn incise_adds_dissection() {
        let mut z = z();
        z.incise(40.0);
        assert!((z.dissection - 40.0).abs() < 1e-3);
    }

    #[test]
    fn incise_clamps_at_max() {
        let mut z = z();
        z.incise(200.0);
        assert!((z.dissection - 100.0).abs() < 1e-3);
    }

    #[test]
    fn incise_fires_just_complete_at_max() {
        let mut z = z();
        z.incise(100.0);
        assert!(z.just_complete);
        assert!(z.is_complete());
    }

    #[test]
    fn incise_no_just_complete_when_already_at_max() {
        let mut z = z();
        z.dissection = 100.0;
        z.incise(10.0);
        assert!(!z.just_complete);
    }

    #[test]
    fn incise_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.incise(50.0);
        assert_eq!(z.dissection, 0.0);
    }

    #[test]
    fn incise_no_op_when_amount_zero() {
        let mut z = z();
        z.incise(0.0);
        assert_eq!(z.dissection, 0.0);
    }

    // --- contaminate ---

    #[test]
    fn contaminate_reduces_dissection() {
        let mut z = z();
        z.dissection = 60.0;
        z.contaminate(20.0);
        assert!((z.dissection - 40.0).abs() < 1e-3);
    }

    #[test]
    fn contaminate_clamps_at_zero() {
        let mut z = z();
        z.dissection = 30.0;
        z.contaminate(200.0);
        assert_eq!(z.dissection, 0.0);
    }

    #[test]
    fn contaminate_fires_just_spoiled_at_zero() {
        let mut z = z();
        z.dissection = 30.0;
        z.contaminate(30.0);
        assert!(z.just_spoiled);
    }

    #[test]
    fn contaminate_no_op_when_already_spoiled() {
        let mut z = z();
        z.contaminate(10.0);
        assert!(!z.just_spoiled);
    }

    #[test]
    fn contaminate_no_op_when_disabled() {
        let mut z = z();
        z.dissection = 50.0;
        z.enabled = false;
        z.contaminate(50.0);
        assert!((z.dissection - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_advances_dissection() {
        let mut z = z(); // rate=1
        z.tick(5.0); // 0 + 1*5 = 5
        assert!((z.dissection - 5.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_complete_on_study_to_max() {
        let mut z = Zootomy::new(100.0, 200.0);
        z.dissection = 95.0;
        z.tick(1.0);
        assert!(z.just_complete);
        assert!(z.is_complete());
    }

    #[test]
    fn tick_no_advance_when_already_complete() {
        let mut z = z();
        z.dissection = 100.0;
        z.tick(1.0);
        assert!(!z.just_complete);
    }

    #[test]
    fn tick_no_advance_when_rate_zero() {
        let mut z = Zootomy::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.dissection, 0.0);
    }

    #[test]
    fn tick_no_advance_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.dissection, 0.0);
    }

    #[test]
    fn tick_clears_just_complete() {
        let mut z = Zootomy::new(100.0, 200.0);
        z.dissection = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_complete);
    }

    #[test]
    fn tick_clears_just_spoiled() {
        let mut z = z();
        z.dissection = 10.0;
        z.contaminate(10.0);
        z.tick(0.016);
        assert!(!z.just_spoiled);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1
        z.tick(7.0); // 1*7 = 7
        assert!((z.dissection - 7.0).abs() < 1e-3);
    }

    // --- is_complete / is_spoiled ---

    #[test]
    fn is_complete_false_when_disabled() {
        let mut z = z();
        z.dissection = 100.0;
        z.enabled = false;
        assert!(!z.is_complete());
    }

    #[test]
    fn is_spoiled_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_spoiled());
    }

    // --- dissection_fraction / effective_understanding ---

    #[test]
    fn dissection_fraction_zero_when_spoiled() {
        assert_eq!(z().dissection_fraction(), 0.0);
    }

    #[test]
    fn dissection_fraction_half_at_midpoint() {
        let mut z = z();
        z.dissection = 50.0;
        assert!((z.dissection_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_understanding_zero_when_spoiled() {
        assert_eq!(z().effective_understanding(100.0), 0.0);
    }

    #[test]
    fn effective_understanding_scales_with_dissection() {
        let mut z = z();
        z.dissection = 75.0;
        assert!((z.effective_understanding(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_understanding_zero_when_disabled() {
        let mut z = z();
        z.dissection = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_understanding(100.0), 0.0);
    }
}
