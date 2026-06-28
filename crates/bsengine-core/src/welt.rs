use bevy_ecs::prelude::Component;

/// Impact-bruise accumulation tracker named after welt, the noun
/// meaning a ridge or bump raised on the flesh by a blow; a
/// heavy blow; a strip or ridge of leather sewn between the
/// upper and sole of a shoe — from the Middle English welte
/// (a strip of leather), from an unknown source, possibly
/// related to the Old Norse völlr (a round staff, a rod), or
/// possibly from the Welsh gwald (a hem, a border). The sense
/// of a raised mark on the skin from a blow — a weal, a
/// bruise-ridge — appears in English from the early sixteenth
/// century and reflects the physical reality of leather-strap
/// punishment: the same strip of leather that bordered a
/// shoe, when applied to the skin with force, left a raised
/// mark of exactly the same shape. The verb to welt means
/// to strike so as to raise welts, and also simply to strike
/// hard; welting is both the craft of attaching welt to a
/// shoe and the act of giving a beating. In the shoemaker's
/// vocabulary, the welt is the structural component that
/// allows a shoe to be resoled: it is the welt that is stitched
/// first to the upper, then to the outsole, creating a durable
/// three-layer seam that can be unstitched and restitched
/// as the sole wears. In game mechanics, a welt mechanic models
/// the slow accumulation of impact marks — the bruises, contusions,
/// welts, and traumatic marks that build on a body or surface
/// as blows land, impacts accumulate, and damage accrues over
/// time. `marks` builds via `strike(amount)` and accumulates
/// passively at `bruise_rate` per second in `tick(dt)` or
/// heals via `salve(amount)`.
///
/// Models impact-bruise fill levels, contusion-saturation bars,
/// trauma-mark accumulators, blow-tally gauges, welt-count
/// fill levels, hit-saturation indicators, bruise-accumulation
/// bars, impact-record meters, wound-mark completion fill
/// levels, or any mechanic where a character, surface, or
/// object slowly accumulates the marks, bruises, or impact
/// records that indicate how much punishment has been absorbed
/// — each blow adding a fraction of damage record until the
/// threshold of serious harm, disability, or breaking point
/// is reached.
///
/// `strike(amount)` adds marks; fires `just_welted` when first
/// reaching `max_marks`. No-op when disabled.
///
/// `salve(amount)` reduces marks immediately; fires `just_healed`
/// when reaching 0. No-op when disabled or already clear.
///
/// `tick(dt)` clears both flags, then increases marks by
/// `bruise_rate * dt` (capped at `max_marks`). Fires
/// `just_welted` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_welted()` returns `marks >= max_marks && enabled`.
///
/// `is_clear()` returns `marks == 0.0` (not gated by `enabled`).
///
/// `mark_fraction()` returns `(marks / max_marks).clamp(0, 1)`.
///
/// `effective_trauma(scale)` returns `scale * mark_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — bruises at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Welt {
    pub marks: f32,
    pub max_marks: f32,
    pub bruise_rate: f32,
    pub just_welted: bool,
    pub just_healed: bool,
    pub enabled: bool,
}

impl Welt {
    pub fn new(max_marks: f32, bruise_rate: f32) -> Self {
        Self {
            marks: 0.0,
            max_marks: max_marks.max(0.1),
            bruise_rate: bruise_rate.max(0.0),
            just_welted: false,
            just_healed: false,
            enabled: true,
        }
    }

    /// Add marks; fires `just_welted` when first reaching max.
    /// No-op when disabled.
    pub fn strike(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.marks < self.max_marks;
        self.marks = (self.marks + amount).min(self.max_marks);
        if was_below && self.marks >= self.max_marks {
            self.just_welted = true;
        }
    }

    /// Reduce marks; fires `just_healed` when reaching 0.
    /// No-op when disabled or already clear.
    pub fn salve(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.marks <= 0.0 {
            return;
        }
        self.marks = (self.marks - amount).max(0.0);
        if self.marks <= 0.0 {
            self.just_healed = true;
        }
    }

    /// Clear flags, then increase marks by `bruise_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_welted = false;
        self.just_healed = false;
        if self.enabled && self.bruise_rate > 0.0 && self.marks < self.max_marks {
            let was_below = self.marks < self.max_marks;
            self.marks = (self.marks + self.bruise_rate * dt).min(self.max_marks);
            if was_below && self.marks >= self.max_marks {
                self.just_welted = true;
            }
        }
    }

    /// `true` when marks are at maximum and component is enabled.
    pub fn is_welted(&self) -> bool {
        self.marks >= self.max_marks && self.enabled
    }

    /// `true` when marks are 0 (not gated by `enabled`).
    pub fn is_clear(&self) -> bool {
        self.marks == 0.0
    }

    /// Fraction of maximum marks [0.0, 1.0].
    pub fn mark_fraction(&self) -> f32 {
        (self.marks / self.max_marks).clamp(0.0, 1.0)
    }

    /// Returns `scale * mark_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_trauma(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.mark_fraction()
    }
}

impl Default for Welt {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Welt {
        Welt::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_clear() {
        let w = w();
        assert_eq!(w.marks, 0.0);
        assert!(w.is_clear());
        assert!(!w.is_welted());
    }

    #[test]
    fn new_clamps_max_marks() {
        let w = Welt::new(-5.0, 1.5);
        assert!((w.max_marks - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_bruise_rate() {
        let w = Welt::new(100.0, -1.5);
        assert_eq!(w.bruise_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Welt::default();
        assert!((w.max_marks - 100.0).abs() < 1e-5);
        assert!((w.bruise_rate - 1.5).abs() < 1e-5);
    }

    // --- strike ---

    #[test]
    fn strike_adds_marks() {
        let mut w = w();
        w.strike(40.0);
        assert!((w.marks - 40.0).abs() < 1e-3);
    }

    #[test]
    fn strike_clamps_at_max() {
        let mut w = w();
        w.strike(200.0);
        assert!((w.marks - 100.0).abs() < 1e-3);
    }

    #[test]
    fn strike_fires_just_welted_at_max() {
        let mut w = w();
        w.strike(100.0);
        assert!(w.just_welted);
        assert!(w.is_welted());
    }

    #[test]
    fn strike_no_just_welted_when_already_at_max() {
        let mut w = w();
        w.marks = 100.0;
        w.strike(10.0);
        assert!(!w.just_welted);
    }

    #[test]
    fn strike_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.strike(50.0);
        assert_eq!(w.marks, 0.0);
    }

    #[test]
    fn strike_no_op_when_amount_zero() {
        let mut w = w();
        w.strike(0.0);
        assert_eq!(w.marks, 0.0);
    }

    // --- salve ---

    #[test]
    fn salve_reduces_marks() {
        let mut w = w();
        w.marks = 60.0;
        w.salve(20.0);
        assert!((w.marks - 40.0).abs() < 1e-3);
    }

    #[test]
    fn salve_clamps_at_zero() {
        let mut w = w();
        w.marks = 30.0;
        w.salve(200.0);
        assert_eq!(w.marks, 0.0);
    }

    #[test]
    fn salve_fires_just_healed_at_zero() {
        let mut w = w();
        w.marks = 30.0;
        w.salve(30.0);
        assert!(w.just_healed);
    }

    #[test]
    fn salve_no_op_when_already_clear() {
        let mut w = w();
        w.salve(10.0);
        assert!(!w.just_healed);
    }

    #[test]
    fn salve_no_op_when_disabled() {
        let mut w = w();
        w.marks = 50.0;
        w.enabled = false;
        w.salve(50.0);
        assert!((w.marks - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_marks() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.marks - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_welted_on_marks_to_max() {
        let mut w = Welt::new(100.0, 200.0);
        w.marks = 95.0;
        w.tick(1.0);
        assert!(w.just_welted);
        assert!(w.is_welted());
    }

    #[test]
    fn tick_no_build_when_already_welted() {
        let mut w = w();
        w.marks = 100.0;
        w.tick(1.0);
        assert!(!w.just_welted);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Welt::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.marks, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.marks, 0.0);
    }

    #[test]
    fn tick_clears_just_welted() {
        let mut w = Welt::new(100.0, 200.0);
        w.marks = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_welted);
    }

    #[test]
    fn tick_clears_just_healed() {
        let mut w = w();
        w.marks = 10.0;
        w.salve(10.0);
        w.tick(0.016);
        assert!(!w.just_healed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.marks - 9.0).abs() < 1e-3);
    }

    // --- is_welted / is_clear ---

    #[test]
    fn is_welted_false_when_disabled() {
        let mut w = w();
        w.marks = 100.0;
        w.enabled = false;
        assert!(!w.is_welted());
    }

    #[test]
    fn is_clear_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_clear());
    }

    // --- mark_fraction / effective_trauma ---

    #[test]
    fn mark_fraction_zero_when_clear() {
        assert_eq!(w().mark_fraction(), 0.0);
    }

    #[test]
    fn mark_fraction_half_at_midpoint() {
        let mut w = w();
        w.marks = 50.0;
        assert!((w.mark_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_trauma_zero_when_clear() {
        assert_eq!(w().effective_trauma(100.0), 0.0);
    }

    #[test]
    fn effective_trauma_scales_with_marks() {
        let mut w = w();
        w.marks = 75.0;
        assert!((w.effective_trauma(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_trauma_zero_when_disabled() {
        let mut w = w();
        w.marks = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_trauma(100.0), 0.0);
    }
}
