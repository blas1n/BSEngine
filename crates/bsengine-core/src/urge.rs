use bevy_ecs::prelude::Component;

/// Goal-state drive that builds while the entity is actively pursuing an
/// objective and decays when it stops. Unlike `Venture` (combat engagement
/// ramp) or `Verve` (action-count triggered), `Urge` is **manually toggled**
/// via `urge_on()` / `urge_off()`: any system that decides the entity is
/// actively pursuing a goal calls `urge_on()`; when the goal lapses it calls
/// `urge_off()`. Tick advances the level, and `effective_drive()` returns a
/// multiplicative bonus proportional to the current urge fraction.
///
/// `urge_on()` — begins building; no-op when already urged or disabled.
///
/// `urge_off()` — stops building; no-op when not urged.
///
/// `tick(dt)` — clears `just_peaked` first; grows `urge_level` by
/// `build_rate * dt` (capped at `max_urge`) when urged; decays by
/// `decay_rate * dt` (floored at 0.0) when not urged; fires `just_peaked`
/// on first reach of `max_urge`; no-op when disabled.
///
/// `is_peaked()` — `urge_level >= max_urge && enabled`.
///
/// `urge_fraction()` — `(urge_level / max_urge).clamp(0.0, 1.0)`.
///
/// `effective_drive(base)` — `base * (1.0 + drive_bonus * urge_fraction())`
/// when enabled; `base` when disabled. Pure query — does not mutate state.
///
/// Distinct from `Venture` (combat-specific engagement ramp started by
/// combat events), `Verve` (action-count driven), `Fervor` (hit-streak),
/// `Fury` (damage-taken reaction), and `Momentum` (speed-continuous): Urge
/// is a **general-purpose goal-drive** manually toggled by any system that
/// tracks whether the entity has an active objective.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Urge {
    /// Current drive level [0.0, max_urge].
    pub urge_level: f32,
    /// Maximum drive level. Clamped >= 1.0.
    pub max_urge: f32,
    /// Build rate per second while urged. Clamped >= 0.0.
    pub build_rate: f32,
    /// Decay rate per second while not urged. Clamped >= 0.0.
    pub decay_rate: f32,
    /// Maximum multiplicative bonus at full urge. Clamped [0.0, 1.0].
    pub drive_bonus: f32,
    /// Whether the entity is currently pursuing its goal.
    pub urged: bool,
    pub just_peaked: bool,
    pub enabled: bool,
}

impl Urge {
    pub fn new(max_urge: f32, build_rate: f32, decay_rate: f32, drive_bonus: f32) -> Self {
        Self {
            urge_level: 0.0,
            max_urge: max_urge.max(1.0),
            build_rate: build_rate.max(0.0),
            decay_rate: decay_rate.max(0.0),
            drive_bonus: drive_bonus.clamp(0.0, 1.0),
            urged: false,
            just_peaked: false,
            enabled: true,
        }
    }

    /// Begin building urge. No-op when already urged or disabled.
    pub fn urge_on(&mut self) {
        if !self.enabled || self.urged {
            return;
        }
        self.urged = true;
    }

    /// Stop building urge. No-op when not urged.
    pub fn urge_off(&mut self) {
        if !self.urged {
            return;
        }
        self.urged = false;
    }

    /// Advance one frame: clear `just_peaked`; build or decay; fire
    /// `just_peaked` on first reach of `max_urge`. No-op when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_peaked = false;

        if !self.enabled {
            return;
        }

        if self.urged {
            let was_below = self.urge_level < self.max_urge;
            self.urge_level = (self.urge_level + self.build_rate * dt).min(self.max_urge);
            if was_below && self.urge_level >= self.max_urge {
                self.just_peaked = true;
            }
        } else {
            self.urge_level = (self.urge_level - self.decay_rate * dt).max(0.0);
        }
    }

    /// `true` when drive has reached its maximum and the component is enabled.
    pub fn is_peaked(&self) -> bool {
        self.urge_level >= self.max_urge && self.enabled
    }

    /// Drive level as a fraction of the maximum [0.0, 1.0].
    pub fn urge_fraction(&self) -> f32 {
        (self.urge_level / self.max_urge).clamp(0.0, 1.0)
    }

    /// Output multiplied by current drive fraction bonus. Pure query.
    pub fn effective_drive(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.drive_bonus * self.urge_fraction())
    }
}

impl Default for Urge {
    fn default() -> Self {
        Self::new(10.0, 2.0, 1.0, 0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_idle() {
        let u = Urge::new(10.0, 2.0, 1.0, 0.3);
        assert_eq!(u.urge_level, 0.0);
        assert!(!u.urged);
        assert!(!u.just_peaked);
        assert!(!u.is_peaked());
    }

    #[test]
    fn urge_on_sets_urged() {
        let mut u = Urge::new(10.0, 2.0, 1.0, 0.3);
        u.urge_on();
        assert!(u.urged);
    }

    #[test]
    fn urge_on_no_op_when_already_urged() {
        let mut u = Urge::new(10.0, 2.0, 1.0, 0.3);
        u.urge_on();
        u.urge_level = 5.0;
        u.urge_on(); // already on, should not reset
        assert_eq!(u.urge_level, 5.0);
    }

    #[test]
    fn urge_on_no_op_when_disabled() {
        let mut u = Urge::new(10.0, 2.0, 1.0, 0.3);
        u.enabled = false;
        u.urge_on();
        assert!(!u.urged);
    }

    #[test]
    fn urge_off_clears_urged() {
        let mut u = Urge::new(10.0, 2.0, 1.0, 0.3);
        u.urge_on();
        u.urge_off();
        assert!(!u.urged);
    }

    #[test]
    fn urge_off_no_op_when_not_urged() {
        let mut u = Urge::new(10.0, 2.0, 1.0, 0.3);
        u.urge_level = 3.0;
        u.urge_off(); // was not on
        assert_eq!(u.urge_level, 3.0);
        assert!(!u.urged);
    }

    #[test]
    fn tick_builds_when_urged() {
        let mut u = Urge::new(10.0, 4.0, 0.0, 0.3);
        u.urge_on();
        u.tick(1.0);
        assert!((u.urge_level - 4.0).abs() < 1e-5);
    }

    #[test]
    fn tick_caps_at_max_urge() {
        let mut u = Urge::new(10.0, 20.0, 0.0, 0.3);
        u.urge_on();
        u.tick(1.0);
        assert!((u.urge_level - 10.0).abs() < 1e-5);
    }

    #[test]
    fn tick_fires_just_peaked_on_first_reach() {
        let mut u = Urge::new(10.0, 10.0, 0.0, 0.3);
        u.urge_on();
        u.tick(1.0);
        assert!(u.just_peaked);
        assert!(u.is_peaked());
    }

    #[test]
    fn tick_no_just_peaked_when_already_at_max() {
        let mut u = Urge::new(10.0, 10.0, 0.0, 0.3);
        u.urge_on();
        u.tick(1.0); // just_peaked fires
        u.tick(1.0); // already peaked, flag cleared
        assert!(!u.just_peaked);
    }

    #[test]
    fn tick_decays_when_not_urged() {
        let mut u = Urge::new(10.0, 0.0, 3.0, 0.3);
        u.urge_level = 6.0;
        u.tick(1.0); // not urged, decay 3.0
        assert!((u.urge_level - 3.0).abs() < 1e-5);
    }

    #[test]
    fn tick_floors_decay_at_zero() {
        let mut u = Urge::new(10.0, 0.0, 10.0, 0.3);
        u.urge_level = 2.0;
        u.tick(1.0);
        assert_eq!(u.urge_level, 0.0);
    }

    #[test]
    fn tick_clears_just_peaked() {
        let mut u = Urge::new(10.0, 10.0, 0.0, 0.3);
        u.urge_on();
        u.tick(1.0); // just_peaked = true
        u.tick(0.016); // should clear
        assert!(!u.just_peaked);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut u = Urge::new(10.0, 5.0, 0.0, 0.3);
        u.urge_on();
        u.enabled = false;
        u.tick(1.0);
        assert_eq!(u.urge_level, 0.0);
    }

    #[test]
    fn is_peaked_true_at_max() {
        let mut u = Urge::new(10.0, 10.0, 0.0, 0.3);
        u.urge_on();
        u.tick(1.0);
        assert!(u.is_peaked());
    }

    #[test]
    fn is_peaked_false_below_max() {
        let u = Urge::new(10.0, 2.0, 0.0, 0.3);
        assert!(!u.is_peaked());
    }

    #[test]
    fn is_peaked_false_when_disabled() {
        let mut u = Urge::new(10.0, 0.0, 0.0, 0.3);
        u.urge_level = 10.0;
        u.enabled = false;
        assert!(!u.is_peaked());
    }

    #[test]
    fn urge_fraction_zero_at_start() {
        let u = Urge::new(10.0, 2.0, 0.0, 0.3);
        assert_eq!(u.urge_fraction(), 0.0);
    }

    #[test]
    fn urge_fraction_half_at_midpoint() {
        let mut u = Urge::new(10.0, 0.0, 0.0, 0.3);
        u.urge_level = 5.0;
        assert!((u.urge_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn urge_fraction_one_at_max() {
        let mut u = Urge::new(10.0, 0.0, 0.0, 0.3);
        u.urge_level = 10.0;
        assert!((u.urge_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_drive_baseline_when_empty() {
        let u = Urge::new(10.0, 0.0, 0.0, 0.5);
        assert!((u.effective_drive(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_drive_at_half_urge() {
        let mut u = Urge::new(10.0, 0.0, 0.0, 0.4);
        u.urge_level = 5.0; // 50% urge
                            // 100 * (1 + 0.4 * 0.5) = 120
        assert!((u.effective_drive(100.0) - 120.0).abs() < 1e-3);
    }

    #[test]
    fn effective_drive_at_full_urge() {
        let mut u = Urge::new(10.0, 0.0, 0.0, 0.5);
        u.urge_level = 10.0;
        // 100 * (1 + 0.5 * 1.0) = 150
        assert!((u.effective_drive(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_drive_base_when_disabled() {
        let mut u = Urge::new(10.0, 0.0, 0.0, 0.5);
        u.urge_level = 10.0;
        u.enabled = false;
        assert!((u.effective_drive(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn max_urge_clamped_to_one() {
        let u = Urge::new(0.0, 2.0, 1.0, 0.3);
        assert!((u.max_urge - 1.0).abs() < 1e-5);
    }

    #[test]
    fn build_rate_clamped_to_zero() {
        let u = Urge::new(10.0, -1.0, 1.0, 0.3);
        assert_eq!(u.build_rate, 0.0);
    }

    #[test]
    fn decay_rate_clamped_to_zero() {
        let u = Urge::new(10.0, 2.0, -1.0, 0.3);
        assert_eq!(u.decay_rate, 0.0);
    }

    #[test]
    fn drive_bonus_clamped_to_one() {
        let u = Urge::new(10.0, 2.0, 1.0, 2.0);
        assert!((u.drive_bonus - 1.0).abs() < 1e-5);
    }

    #[test]
    fn drive_bonus_clamped_to_zero() {
        let u = Urge::new(10.0, 2.0, 1.0, -0.5);
        assert_eq!(u.drive_bonus, 0.0);
    }

    #[test]
    fn on_off_cycle() {
        let mut u = Urge::new(10.0, 5.0, 2.0, 0.3);
        u.urge_on();
        u.tick(1.0); // level = 5.0
        u.urge_off();
        u.tick(1.0); // decay 2.0 → 3.0
        assert!((u.urge_level - 3.0).abs() < 1e-4);
        assert!(!u.urged);
    }

    #[test]
    fn build_cycle_peaks_and_decays() {
        let mut u = Urge::new(10.0, 10.0, 5.0, 0.3);
        u.urge_on();
        u.tick(1.0); // just_peaked, level = 10
        assert!(u.just_peaked);
        u.urge_off();
        u.tick(1.0); // level = 5
        assert!((u.urge_level - 5.0).abs() < 1e-4);
        assert!(!u.is_peaked());
    }

    #[test]
    fn zero_build_rate_never_peaks() {
        let mut u = Urge::new(10.0, 0.0, 0.0, 0.3);
        u.urge_on();
        u.tick(100.0);
        assert!(!u.is_peaked());
        assert_eq!(u.urge_level, 0.0);
    }

    #[test]
    fn zero_decay_rate_holds_level() {
        let mut u = Urge::new(10.0, 5.0, 0.0, 0.3);
        u.urge_on();
        u.tick(1.0); // level = 5
        u.urge_off();
        u.tick(10.0); // no decay
        assert!((u.urge_level - 5.0).abs() < 1e-4);
    }
}
