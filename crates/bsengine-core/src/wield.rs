use bevy_ecs::prelude::Component;

/// Weapon-mastery accumulation tracker named after wield, the verb
/// meaning to handle (a weapon or tool) effectively; to exert or
/// exercise (authority or influence) — from the Old English
/// wieldan, gewieldan (to govern, to control, to have power over),
/// from the Proto-Germanic waldaną (to rule, to command), from
/// the Proto-Indo-European root wal- (to be strong, to rule).
/// The root wal- spread widely: it gave Latin valere (to be
/// strong), valeo (I am well), and through them the English
/// valid, value, and valor. It gave the Germanic languages
/// wield, weld (to govern, now surviving only in welding metal),
/// and the Scandinavian names Sigvald, Harald, and others that
/// contain the -wald element meaning "ruler." To wield a sword
/// is not simply to hold it but to control it — the distinction
/// the verb insists upon is between passive possession and active
/// command, between having a weapon in your hand and having it
/// in your power. The soldier who cannot wield his sword is
/// holding a heavy piece of metal; the soldier who wields it
/// is an extension of the blade. In political usage, to wield
/// power or authority is similarly to exercise it with effect —
/// not merely to possess the title but to make the title mean
/// something. A monarch who cannot wield power is a figurehead;
/// a minister who wields power has made it active, directed,
/// and consequential. In game mechanics, a wield mechanic
/// models the slow build of weapon mastery or tool proficiency
/// — the accumulation of familiarity, technique, and responsive
/// control that eventually reaches the threshold of true mastery.
/// `mastery` builds via `practise(amount)` and accumulates
/// passively at `train_rate` per second in `tick(dt)` or
/// fades via `fumble(amount)`.
///
/// Models weapon-mastery fill levels, proficiency-saturation
/// bars, skill-familiarity accumulators, tool-control gauges,
/// combat-technique fill levels, discipline-saturation
/// indicators, muscle-memory accumulation bars, grip-confidence
/// meters, specialisation-completion fill levels, or any
/// mechanic where a character slowly accumulates the mastery
/// required to use a weapon, tool, or ability at its full
/// potential — each repetition adding a fraction of refinement
/// until the threshold of fluency is crossed and the weapon
/// and the wielder become, for the first time, one.
///
/// `practise(amount)` adds mastery; fires `just_mastered` when
/// first reaching `max_mastery`. No-op when disabled.
///
/// `fumble(amount)` reduces mastery immediately; fires
/// `just_disarmed` when reaching 0. No-op when disabled or
/// already disarmed.
///
/// `tick(dt)` clears both flags, then increases mastery by
/// `train_rate * dt` (capped at `max_mastery`). Fires
/// `just_mastered` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_mastered()` returns `mastery >= max_mastery && enabled`.
///
/// `is_disarmed()` returns `mastery == 0.0` (not gated by
/// `enabled`).
///
/// `mastery_fraction()` returns
/// `(mastery / max_mastery).clamp(0, 1)`.
///
/// `effective_control(scale)` returns `scale * mastery_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — trains at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wield {
    pub mastery: f32,
    pub max_mastery: f32,
    pub train_rate: f32,
    pub just_mastered: bool,
    pub just_disarmed: bool,
    pub enabled: bool,
}

impl Wield {
    pub fn new(max_mastery: f32, train_rate: f32) -> Self {
        Self {
            mastery: 0.0,
            max_mastery: max_mastery.max(0.1),
            train_rate: train_rate.max(0.0),
            just_mastered: false,
            just_disarmed: false,
            enabled: true,
        }
    }

    /// Add mastery; fires `just_mastered` when first reaching max.
    /// No-op when disabled.
    pub fn practise(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.mastery < self.max_mastery;
        self.mastery = (self.mastery + amount).min(self.max_mastery);
        if was_below && self.mastery >= self.max_mastery {
            self.just_mastered = true;
        }
    }

    /// Reduce mastery; fires `just_disarmed` when reaching 0.
    /// No-op when disabled or already disarmed.
    pub fn fumble(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.mastery <= 0.0 {
            return;
        }
        self.mastery = (self.mastery - amount).max(0.0);
        if self.mastery <= 0.0 {
            self.just_disarmed = true;
        }
    }

    /// Clear flags, then increase mastery by `train_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_mastered = false;
        self.just_disarmed = false;
        if self.enabled && self.train_rate > 0.0 && self.mastery < self.max_mastery {
            let was_below = self.mastery < self.max_mastery;
            self.mastery = (self.mastery + self.train_rate * dt).min(self.max_mastery);
            if was_below && self.mastery >= self.max_mastery {
                self.just_mastered = true;
            }
        }
    }

    /// `true` when mastery is at maximum and component is enabled.
    pub fn is_mastered(&self) -> bool {
        self.mastery >= self.max_mastery && self.enabled
    }

    /// `true` when mastery is 0 (not gated by `enabled`).
    pub fn is_disarmed(&self) -> bool {
        self.mastery == 0.0
    }

    /// Fraction of maximum mastery [0.0, 1.0].
    pub fn mastery_fraction(&self) -> f32 {
        (self.mastery / self.max_mastery).clamp(0.0, 1.0)
    }

    /// Returns `scale * mastery_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_control(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.mastery_fraction()
    }
}

impl Default for Wield {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wield {
        Wield::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_disarmed() {
        let w = w();
        assert_eq!(w.mastery, 0.0);
        assert!(w.is_disarmed());
        assert!(!w.is_mastered());
    }

    #[test]
    fn new_clamps_max_mastery() {
        let w = Wield::new(-5.0, 1.5);
        assert!((w.max_mastery - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_train_rate() {
        let w = Wield::new(100.0, -1.5);
        assert_eq!(w.train_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Wield::default();
        assert!((w.max_mastery - 100.0).abs() < 1e-5);
        assert!((w.train_rate - 1.5).abs() < 1e-5);
    }

    // --- practise ---

    #[test]
    fn practise_adds_mastery() {
        let mut w = w();
        w.practise(40.0);
        assert!((w.mastery - 40.0).abs() < 1e-3);
    }

    #[test]
    fn practise_clamps_at_max() {
        let mut w = w();
        w.practise(200.0);
        assert!((w.mastery - 100.0).abs() < 1e-3);
    }

    #[test]
    fn practise_fires_just_mastered_at_max() {
        let mut w = w();
        w.practise(100.0);
        assert!(w.just_mastered);
        assert!(w.is_mastered());
    }

    #[test]
    fn practise_no_just_mastered_when_already_at_max() {
        let mut w = w();
        w.mastery = 100.0;
        w.practise(10.0);
        assert!(!w.just_mastered);
    }

    #[test]
    fn practise_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.practise(50.0);
        assert_eq!(w.mastery, 0.0);
    }

    #[test]
    fn practise_no_op_when_amount_zero() {
        let mut w = w();
        w.practise(0.0);
        assert_eq!(w.mastery, 0.0);
    }

    // --- fumble ---

    #[test]
    fn fumble_reduces_mastery() {
        let mut w = w();
        w.mastery = 60.0;
        w.fumble(20.0);
        assert!((w.mastery - 40.0).abs() < 1e-3);
    }

    #[test]
    fn fumble_clamps_at_zero() {
        let mut w = w();
        w.mastery = 30.0;
        w.fumble(200.0);
        assert_eq!(w.mastery, 0.0);
    }

    #[test]
    fn fumble_fires_just_disarmed_at_zero() {
        let mut w = w();
        w.mastery = 30.0;
        w.fumble(30.0);
        assert!(w.just_disarmed);
    }

    #[test]
    fn fumble_no_op_when_already_disarmed() {
        let mut w = w();
        w.fumble(10.0);
        assert!(!w.just_disarmed);
    }

    #[test]
    fn fumble_no_op_when_disabled() {
        let mut w = w();
        w.mastery = 50.0;
        w.enabled = false;
        w.fumble(50.0);
        assert!((w.mastery - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_mastery() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.mastery - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_mastered_on_mastery_to_max() {
        let mut w = Wield::new(100.0, 200.0);
        w.mastery = 95.0;
        w.tick(1.0);
        assert!(w.just_mastered);
        assert!(w.is_mastered());
    }

    #[test]
    fn tick_no_build_when_already_mastered() {
        let mut w = w();
        w.mastery = 100.0;
        w.tick(1.0);
        assert!(!w.just_mastered);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Wield::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.mastery, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.mastery, 0.0);
    }

    #[test]
    fn tick_clears_just_mastered() {
        let mut w = Wield::new(100.0, 200.0);
        w.mastery = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_mastered);
    }

    #[test]
    fn tick_clears_just_disarmed() {
        let mut w = w();
        w.mastery = 10.0;
        w.fumble(10.0);
        w.tick(0.016);
        assert!(!w.just_disarmed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.mastery - 9.0).abs() < 1e-3);
    }

    // --- is_mastered / is_disarmed ---

    #[test]
    fn is_mastered_false_when_disabled() {
        let mut w = w();
        w.mastery = 100.0;
        w.enabled = false;
        assert!(!w.is_mastered());
    }

    #[test]
    fn is_disarmed_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_disarmed());
    }

    // --- mastery_fraction / effective_control ---

    #[test]
    fn mastery_fraction_zero_when_disarmed() {
        assert_eq!(w().mastery_fraction(), 0.0);
    }

    #[test]
    fn mastery_fraction_half_at_midpoint() {
        let mut w = w();
        w.mastery = 50.0;
        assert!((w.mastery_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_control_zero_when_disarmed() {
        assert_eq!(w().effective_control(100.0), 0.0);
    }

    #[test]
    fn effective_control_scales_with_mastery() {
        let mut w = w();
        w.mastery = 75.0;
        assert!((w.effective_control(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_control_zero_when_disabled() {
        let mut w = w();
        w.mastery = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_control(100.0), 0.0);
    }
}
