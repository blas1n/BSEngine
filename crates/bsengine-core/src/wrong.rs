use bevy_ecs::prelude::Component;

/// Fault-error accumulation tracker named after wrong, the
/// adjective and noun meaning not in accordance with fact,
/// standard, or morality; not correct or suitable; an
/// injustice or harmful act — from the Old Norse rangr
/// (crooked, awry, unjust), from the Proto-Germanic
/// wrangaz (crooked, wrong), from the Proto-Indo-European
/// root wrengh- (to turn, to twist), the same root that
/// gave wring, wrench, and wrist. The original sense is
/// physical and geometric: wrong was crooked, bent out
/// of true, turned askew — the opposite of straight and
/// right. The Old English equivalent was wrang, and the
/// word entered English through contact with Old Norse
/// speakers in the Danelaw, displacing native English
/// terms. The moral sense follows naturally from the
/// physical: a wrong is a deviation from the straight
/// line of justice, truth, or proper conduct. In law,
/// a wrong is an act that violates another's rights;
/// in ethics, it is an act contrary to moral standards;
/// in cognition, it is a conclusion that deviates from
/// reality. The connection between geometric crookedness
/// and moral crookedness is ancient and cross-cultural
/// — rectitude (from Latin rectus, straight) and right
/// (from Old English riht, straight) share the same
/// metaphor. In game mechanics, a wrong mechanic models
/// the accumulation of faults, errors, misjudgments,
/// or offenses — the build of wrongness that eventually
/// crosses the threshold at which a character is judged
/// guilty, a system flags an error state, a reputation
/// is ruined, or a sentence is imposed. `faults` builds
/// via `err(amount)` and accumulates passively at
/// `err_rate` per second in `tick(dt)` or is corrected
/// via `correct(amount)`.
///
/// Models fault-accumulation fill levels, guilt-saturation
/// bars, error-record trackers, infraction-build gauges,
/// offense-tally fill levels, transgression-saturation
/// indicators, misconduct-accumulation bars, culpability
/// meters, judgment-completion fill levels, or any mechanic
/// where a character, system, or entity slowly accumulates
/// the faults, errors, or violations that lead to judgment,
/// penalty, or a triggered consequence.
///
/// `err(amount)` adds faults; fires `just_wrong` when first
/// reaching `max_faults`. No-op when disabled.
///
/// `correct(amount)` reduces faults immediately; fires
/// `just_right` when reaching 0. No-op when disabled or
/// already right.
///
/// `tick(dt)` clears both flags, then increases faults by
/// `err_rate * dt` (capped at `max_faults`). Fires
/// `just_wrong` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_wrong()` returns `faults >= max_faults && enabled`.
///
/// `is_right()` returns `faults == 0.0` (not gated by
/// `enabled`).
///
/// `fault_fraction()` returns
/// `(faults / max_faults).clamp(0, 1)`.
///
/// `effective_guilt(scale)` returns `scale * fault_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — errs at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wrong {
    pub faults: f32,
    pub max_faults: f32,
    pub err_rate: f32,
    pub just_wrong: bool,
    pub just_right: bool,
    pub enabled: bool,
}

impl Wrong {
    pub fn new(max_faults: f32, err_rate: f32) -> Self {
        Self {
            faults: 0.0,
            max_faults: max_faults.max(0.1),
            err_rate: err_rate.max(0.0),
            just_wrong: false,
            just_right: false,
            enabled: true,
        }
    }

    /// Add faults; fires `just_wrong` when first reaching max.
    /// No-op when disabled.
    pub fn err(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.faults < self.max_faults;
        self.faults = (self.faults + amount).min(self.max_faults);
        if was_below && self.faults >= self.max_faults {
            self.just_wrong = true;
        }
    }

    /// Reduce faults; fires `just_right` when reaching 0.
    /// No-op when disabled or already right.
    pub fn correct(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.faults <= 0.0 {
            return;
        }
        self.faults = (self.faults - amount).max(0.0);
        if self.faults <= 0.0 {
            self.just_right = true;
        }
    }

    /// Clear flags, then increase faults by `err_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_wrong = false;
        self.just_right = false;
        if self.enabled && self.err_rate > 0.0 && self.faults < self.max_faults {
            let was_below = self.faults < self.max_faults;
            self.faults = (self.faults + self.err_rate * dt).min(self.max_faults);
            if was_below && self.faults >= self.max_faults {
                self.just_wrong = true;
            }
        }
    }

    /// `true` when faults are at maximum and component is enabled.
    pub fn is_wrong(&self) -> bool {
        self.faults >= self.max_faults && self.enabled
    }

    /// `true` when faults are 0 (not gated by `enabled`).
    pub fn is_right(&self) -> bool {
        self.faults == 0.0
    }

    /// Fraction of maximum faults [0.0, 1.0].
    pub fn fault_fraction(&self) -> f32 {
        (self.faults / self.max_faults).clamp(0.0, 1.0)
    }

    /// Returns `scale * fault_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_guilt(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.fault_fraction()
    }
}

impl Default for Wrong {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wrong {
        Wrong::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_right() {
        let w = w();
        assert_eq!(w.faults, 0.0);
        assert!(w.is_right());
        assert!(!w.is_wrong());
    }

    #[test]
    fn new_clamps_max_faults() {
        let w = Wrong::new(-5.0, 1.5);
        assert!((w.max_faults - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_err_rate() {
        let w = Wrong::new(100.0, -1.5);
        assert_eq!(w.err_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Wrong::default();
        assert!((w.max_faults - 100.0).abs() < 1e-5);
        assert!((w.err_rate - 1.5).abs() < 1e-5);
    }

    // --- err ---

    #[test]
    fn err_adds_faults() {
        let mut w = w();
        w.err(40.0);
        assert!((w.faults - 40.0).abs() < 1e-3);
    }

    #[test]
    fn err_clamps_at_max() {
        let mut w = w();
        w.err(200.0);
        assert!((w.faults - 100.0).abs() < 1e-3);
    }

    #[test]
    fn err_fires_just_wrong_at_max() {
        let mut w = w();
        w.err(100.0);
        assert!(w.just_wrong);
        assert!(w.is_wrong());
    }

    #[test]
    fn err_no_just_wrong_when_already_at_max() {
        let mut w = w();
        w.faults = 100.0;
        w.err(10.0);
        assert!(!w.just_wrong);
    }

    #[test]
    fn err_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.err(50.0);
        assert_eq!(w.faults, 0.0);
    }

    #[test]
    fn err_no_op_when_amount_zero() {
        let mut w = w();
        w.err(0.0);
        assert_eq!(w.faults, 0.0);
    }

    // --- correct ---

    #[test]
    fn correct_reduces_faults() {
        let mut w = w();
        w.faults = 60.0;
        w.correct(20.0);
        assert!((w.faults - 40.0).abs() < 1e-3);
    }

    #[test]
    fn correct_clamps_at_zero() {
        let mut w = w();
        w.faults = 30.0;
        w.correct(200.0);
        assert_eq!(w.faults, 0.0);
    }

    #[test]
    fn correct_fires_just_right_at_zero() {
        let mut w = w();
        w.faults = 30.0;
        w.correct(30.0);
        assert!(w.just_right);
    }

    #[test]
    fn correct_no_op_when_already_right() {
        let mut w = w();
        w.correct(10.0);
        assert!(!w.just_right);
    }

    #[test]
    fn correct_no_op_when_disabled() {
        let mut w = w();
        w.faults = 50.0;
        w.enabled = false;
        w.correct(50.0);
        assert!((w.faults - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_faults() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.faults - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_wrong_on_faults_to_max() {
        let mut w = Wrong::new(100.0, 200.0);
        w.faults = 95.0;
        w.tick(1.0);
        assert!(w.just_wrong);
        assert!(w.is_wrong());
    }

    #[test]
    fn tick_no_build_when_already_wrong() {
        let mut w = w();
        w.faults = 100.0;
        w.tick(1.0);
        assert!(!w.just_wrong);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Wrong::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.faults, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.faults, 0.0);
    }

    #[test]
    fn tick_clears_just_wrong() {
        let mut w = Wrong::new(100.0, 200.0);
        w.faults = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_wrong);
    }

    #[test]
    fn tick_clears_just_right() {
        let mut w = w();
        w.faults = 10.0;
        w.correct(10.0);
        w.tick(0.016);
        assert!(!w.just_right);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.faults - 9.0).abs() < 1e-3);
    }

    // --- is_wrong / is_right ---

    #[test]
    fn is_wrong_false_when_disabled() {
        let mut w = w();
        w.faults = 100.0;
        w.enabled = false;
        assert!(!w.is_wrong());
    }

    #[test]
    fn is_right_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_right());
    }

    // --- fault_fraction / effective_guilt ---

    #[test]
    fn fault_fraction_zero_when_right() {
        assert_eq!(w().fault_fraction(), 0.0);
    }

    #[test]
    fn fault_fraction_half_at_midpoint() {
        let mut w = w();
        w.faults = 50.0;
        assert!((w.fault_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_guilt_zero_when_right() {
        assert_eq!(w().effective_guilt(100.0), 0.0);
    }

    #[test]
    fn effective_guilt_scales_with_faults() {
        let mut w = w();
        w.faults = 75.0;
        assert!((w.effective_guilt(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_guilt_zero_when_disabled() {
        let mut w = w();
        w.faults = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_guilt(100.0), 0.0);
    }
}
