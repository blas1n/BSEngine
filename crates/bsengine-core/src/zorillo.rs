use bevy_ecs::prelude::Component;

/// Mustelid territory-scent tracker named after the zorillo — the
/// South American striped skunk-polecat (also an alternate form of
/// zorilla, the African striped polecat). `musk` builds via
/// `mark(amount)` and accumulates passively at `mark_rate` per
/// second in `tick(dt)` or dissipates via `disperse(amount)`.
///
/// Models mustelid-territory saturation fill levels, scent-marking
/// radius-coverage bars, olfactory-deterrence intensity gauges,
/// pheromone-boundary establishment trackers, anal-gland discharge
/// pressure meters, skunk-spray projectile charge bars, musk-deer
/// gland fill levels, weasel-scent-trail intensity trackers,
/// territorial-musk-investment accumulators, or any mechanic where
/// a small, striped, magnificently-defended creature sprays a
/// precisely-aimed stream of sulphurous compounds at whatever
/// approached it despite repeated warnings — until every intruder
/// within a twenty-meter radius has retreated, every tree root
/// and rock face bears its chemical signature, and the territory
/// is comprehensively and unmistakably claimed.
///
/// `mark(amount)` adds musk; fires `just_marked` when first
/// reaching `max_musk`. No-op when disabled.
///
/// `disperse(amount)` reduces musk immediately; fires
/// `just_neutral` when reaching 0. No-op when disabled or
/// already neutral.
///
/// `tick(dt)` clears both flags, then increases musk by
/// `mark_rate * dt` (capped at `max_musk`). Fires `just_marked`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_marked()` returns `musk >= max_musk && enabled`.
///
/// `is_neutral()` returns `musk == 0.0` (not gated by `enabled`).
///
/// `musk_fraction()` returns `(musk / max_musk).clamp(0, 1)`.
///
/// `effective_deterrence(scale)` returns `scale * musk_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — marks at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zorillo {
    pub musk: f32,
    pub max_musk: f32,
    pub mark_rate: f32,
    pub just_marked: bool,
    pub just_neutral: bool,
    pub enabled: bool,
}

impl Zorillo {
    pub fn new(max_musk: f32, mark_rate: f32) -> Self {
        Self {
            musk: 0.0,
            max_musk: max_musk.max(0.1),
            mark_rate: mark_rate.max(0.0),
            just_marked: false,
            just_neutral: false,
            enabled: true,
        }
    }

    /// Add musk; fires `just_marked` when first reaching max.
    /// No-op when disabled.
    pub fn mark(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.musk < self.max_musk;
        self.musk = (self.musk + amount).min(self.max_musk);
        if was_below && self.musk >= self.max_musk {
            self.just_marked = true;
        }
    }

    /// Reduce musk; fires `just_neutral` when reaching 0.
    /// No-op when disabled or already neutral.
    pub fn disperse(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.musk <= 0.0 {
            return;
        }
        self.musk = (self.musk - amount).max(0.0);
        if self.musk <= 0.0 {
            self.just_neutral = true;
        }
    }

    /// Clear flags, then increase musk by `mark_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_marked = false;
        self.just_neutral = false;
        if self.enabled && self.mark_rate > 0.0 && self.musk < self.max_musk {
            let was_below = self.musk < self.max_musk;
            self.musk = (self.musk + self.mark_rate * dt).min(self.max_musk);
            if was_below && self.musk >= self.max_musk {
                self.just_marked = true;
            }
        }
    }

    /// `true` when musk is at maximum and component is enabled.
    pub fn is_marked(&self) -> bool {
        self.musk >= self.max_musk && self.enabled
    }

    /// `true` when musk is 0 (not gated by `enabled`).
    pub fn is_neutral(&self) -> bool {
        self.musk == 0.0
    }

    /// Fraction of maximum musk [0.0, 1.0].
    pub fn musk_fraction(&self) -> f32 {
        (self.musk / self.max_musk).clamp(0.0, 1.0)
    }

    /// Returns `scale * musk_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_deterrence(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.musk_fraction()
    }
}

impl Default for Zorillo {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zorillo {
        Zorillo::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_neutral() {
        let z = z();
        assert_eq!(z.musk, 0.0);
        assert!(z.is_neutral());
        assert!(!z.is_marked());
    }

    #[test]
    fn new_clamps_max_musk() {
        let z = Zorillo::new(-5.0, 1.5);
        assert!((z.max_musk - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_mark_rate() {
        let z = Zorillo::new(100.0, -1.5);
        assert_eq!(z.mark_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zorillo::default();
        assert!((z.max_musk - 100.0).abs() < 1e-5);
        assert!((z.mark_rate - 1.5).abs() < 1e-5);
    }

    // --- mark ---

    #[test]
    fn mark_adds_musk() {
        let mut z = z();
        z.mark(40.0);
        assert!((z.musk - 40.0).abs() < 1e-3);
    }

    #[test]
    fn mark_clamps_at_max() {
        let mut z = z();
        z.mark(200.0);
        assert!((z.musk - 100.0).abs() < 1e-3);
    }

    #[test]
    fn mark_fires_just_marked_at_max() {
        let mut z = z();
        z.mark(100.0);
        assert!(z.just_marked);
        assert!(z.is_marked());
    }

    #[test]
    fn mark_no_just_marked_when_already_at_max() {
        let mut z = z();
        z.musk = 100.0;
        z.mark(10.0);
        assert!(!z.just_marked);
    }

    #[test]
    fn mark_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.mark(50.0);
        assert_eq!(z.musk, 0.0);
    }

    #[test]
    fn mark_no_op_when_amount_zero() {
        let mut z = z();
        z.mark(0.0);
        assert_eq!(z.musk, 0.0);
    }

    // --- disperse ---

    #[test]
    fn disperse_reduces_musk() {
        let mut z = z();
        z.musk = 60.0;
        z.disperse(20.0);
        assert!((z.musk - 40.0).abs() < 1e-3);
    }

    #[test]
    fn disperse_clamps_at_zero() {
        let mut z = z();
        z.musk = 30.0;
        z.disperse(200.0);
        assert_eq!(z.musk, 0.0);
    }

    #[test]
    fn disperse_fires_just_neutral_at_zero() {
        let mut z = z();
        z.musk = 30.0;
        z.disperse(30.0);
        assert!(z.just_neutral);
    }

    #[test]
    fn disperse_no_op_when_already_neutral() {
        let mut z = z();
        z.disperse(10.0);
        assert!(!z.just_neutral);
    }

    #[test]
    fn disperse_no_op_when_disabled() {
        let mut z = z();
        z.musk = 50.0;
        z.enabled = false;
        z.disperse(50.0);
        assert!((z.musk - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_marks_musk() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.musk - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_marked_on_mark_to_max() {
        let mut z = Zorillo::new(100.0, 200.0);
        z.musk = 95.0;
        z.tick(1.0);
        assert!(z.just_marked);
        assert!(z.is_marked());
    }

    #[test]
    fn tick_no_mark_when_already_marked() {
        let mut z = z();
        z.musk = 100.0;
        z.tick(1.0);
        assert!(!z.just_marked);
    }

    #[test]
    fn tick_no_mark_when_rate_zero() {
        let mut z = Zorillo::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.musk, 0.0);
    }

    #[test]
    fn tick_no_mark_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.musk, 0.0);
    }

    #[test]
    fn tick_clears_just_marked() {
        let mut z = Zorillo::new(100.0, 200.0);
        z.musk = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_marked);
    }

    #[test]
    fn tick_clears_just_neutral() {
        let mut z = z();
        z.musk = 10.0;
        z.disperse(10.0);
        z.tick(0.016);
        assert!(!z.just_neutral);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.musk - 9.0).abs() < 1e-3);
    }

    // --- is_marked / is_neutral ---

    #[test]
    fn is_marked_false_when_disabled() {
        let mut z = z();
        z.musk = 100.0;
        z.enabled = false;
        assert!(!z.is_marked());
    }

    #[test]
    fn is_neutral_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_neutral());
    }

    // --- musk_fraction / effective_deterrence ---

    #[test]
    fn musk_fraction_zero_when_neutral() {
        assert_eq!(z().musk_fraction(), 0.0);
    }

    #[test]
    fn musk_fraction_half_at_midpoint() {
        let mut z = z();
        z.musk = 50.0;
        assert!((z.musk_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_deterrence_zero_when_neutral() {
        assert_eq!(z().effective_deterrence(100.0), 0.0);
    }

    #[test]
    fn effective_deterrence_scales_with_musk() {
        let mut z = z();
        z.musk = 75.0;
        assert!((z.effective_deterrence(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_deterrence_zero_when_disabled() {
        let mut z = z();
        z.musk = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_deterrence(100.0), 0.0);
    }
}
