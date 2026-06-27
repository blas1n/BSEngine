use bevy_ecs::prelude::Component;

/// Pursuit-escalation mechanic: while actively hounding a target the entity's
/// effective speed increases over time, making prolonged chases progressively
/// harder to escape.
///
/// `lock_on()` starts the pursuit and fires `just_locked`. `lose_target()`
/// ends it and fires `just_lost`. `tick(dt)` increments the chase timer while
/// active (no decrement on loss — the timer resets in `lose_target()`), and
/// clears one-frame flags at the start of each call.
///
/// `effective_speed(base)` returns `base * (1 + speed_bonus())` while
/// hunting; `speed_bonus()` grows linearly with `chase_timer` up to
/// `max_escalation`.
///
/// Distinct from `Follow` (generic target-following locomotion), `Patrol`
/// (route-based movement), and `Homing` (position-seeking missile guidance):
/// Hound is a **pursuit-escalation mechanic** — the longer the chase, the
/// faster and more relentless the pursuer becomes.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Hound {
    pub active: bool,
    /// Accumulated time spent actively chasing, in seconds. Resets on
    /// `lose_target()`.
    pub chase_timer: f32,
    /// Speed bonus gained per second of active chase. Clamped ≥ 0.0.
    pub escalation_rate: f32,
    /// Maximum speed bonus fraction (e.g. 0.5 = +50% speed cap). Clamped ≥ 0.0.
    pub max_escalation: f32,
    pub just_locked: bool,
    pub just_lost: bool,
    pub enabled: bool,
}

impl Hound {
    pub fn new(escalation_rate: f32, max_escalation: f32) -> Self {
        Self {
            active: false,
            chase_timer: 0.0,
            escalation_rate: escalation_rate.max(0.0),
            max_escalation: max_escalation.max(0.0),
            just_locked: false,
            just_lost: false,
            enabled: true,
        }
    }

    /// Begin pursuit. Fires `just_locked`. No-op when already hunting or disabled.
    pub fn lock_on(&mut self) {
        if self.active || !self.enabled {
            return;
        }
        self.active = true;
        self.just_locked = true;
    }

    /// End pursuit. Fires `just_lost` and resets `chase_timer`. No-op when
    /// not currently hunting.
    pub fn lose_target(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        self.chase_timer = 0.0;
        self.just_lost = true;
    }

    /// Advance the chase timer when active, then clear one-frame flags.
    pub fn tick(&mut self, dt: f32) {
        self.just_locked = false;
        self.just_lost = false;

        if self.active {
            self.chase_timer += dt;
        }
    }

    /// `true` while actively pursuing and enabled.
    pub fn is_hunting(&self) -> bool {
        self.active && self.enabled
    }

    /// Current speed bonus fraction [0.0, `max_escalation`]. Returns 0.0
    /// when not hunting.
    pub fn speed_bonus(&self) -> f32 {
        if !self.is_hunting() {
            return 0.0;
        }
        (self.chase_timer * self.escalation_rate).min(self.max_escalation)
    }

    /// Effective movement speed after escalation bonus.
    /// Returns `base * (1 + speed_bonus())` while hunting; `base` otherwise.
    pub fn effective_speed(&self, base: f32) -> f32 {
        base * (1.0 + self.speed_bonus())
    }

    /// Escalation progress [0.0 = no bonus, 1.0 = fully maxed]. Returns 0.0
    /// when `max_escalation == 0` or not hunting.
    pub fn escalation_fraction(&self) -> f32 {
        if self.max_escalation <= 0.0 || !self.is_hunting() {
            return 0.0;
        }
        (self.speed_bonus() / self.max_escalation).clamp(0.0, 1.0)
    }
}

impl Default for Hound {
    fn default() -> Self {
        Self::new(0.05, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_not_hunting() {
        let h = Hound::new(0.05, 0.5);
        assert!(!h.is_hunting());
        assert_eq!(h.chase_timer, 0.0);
    }

    #[test]
    fn lock_on_activates() {
        let mut h = Hound::new(0.05, 0.5);
        h.lock_on();
        assert!(h.is_hunting());
        assert!(h.just_locked);
    }

    #[test]
    fn lock_on_no_op_when_already_hunting() {
        let mut h = Hound::new(0.05, 0.5);
        h.lock_on();
        h.tick(1.0);
        h.lock_on();
        assert!(!h.just_locked);
    }

    #[test]
    fn lock_on_no_op_when_disabled() {
        let mut h = Hound::new(0.05, 0.5);
        h.enabled = false;
        h.lock_on();
        assert!(!h.active);
    }

    #[test]
    fn lose_target_deactivates() {
        let mut h = Hound::new(0.05, 0.5);
        h.lock_on();
        h.lose_target();
        assert!(!h.is_hunting());
        assert!(h.just_lost);
        assert_eq!(h.chase_timer, 0.0);
    }

    #[test]
    fn lose_target_no_op_when_not_hunting() {
        let mut h = Hound::new(0.05, 0.5);
        h.lose_target();
        assert!(!h.just_lost);
    }

    #[test]
    fn lose_target_resets_chase_timer() {
        let mut h = Hound::new(0.05, 0.5);
        h.lock_on();
        h.tick(5.0);
        h.lose_target();
        assert_eq!(h.chase_timer, 0.0);
    }

    #[test]
    fn tick_increments_timer_when_hunting() {
        let mut h = Hound::new(0.05, 0.5);
        h.lock_on();
        h.tick(2.0);
        assert!((h.chase_timer - 2.0).abs() < 1e-5);
    }

    #[test]
    fn tick_no_increment_when_not_hunting() {
        let mut h = Hound::new(0.05, 0.5);
        h.tick(5.0);
        assert_eq!(h.chase_timer, 0.0);
    }

    #[test]
    fn tick_clears_just_locked() {
        let mut h = Hound::new(0.05, 0.5);
        h.lock_on();
        h.tick(0.016);
        assert!(!h.just_locked);
    }

    #[test]
    fn tick_clears_just_lost() {
        let mut h = Hound::new(0.05, 0.5);
        h.lock_on();
        h.lose_target();
        h.tick(0.016);
        assert!(!h.just_lost);
    }

    #[test]
    fn speed_bonus_zero_when_not_hunting() {
        let h = Hound::new(0.1, 0.5);
        assert!((h.speed_bonus()).abs() < 1e-5);
    }

    #[test]
    fn speed_bonus_grows_with_time() {
        let mut h = Hound::new(0.1, 1.0);
        h.lock_on();
        h.tick(3.0); // 0.1 * 3 = 0.3
        assert!((h.speed_bonus() - 0.3).abs() < 1e-4);
    }

    #[test]
    fn speed_bonus_capped_at_max_escalation() {
        let mut h = Hound::new(0.1, 0.5);
        h.lock_on();
        h.tick(100.0); // would be 10.0 uncapped
        assert!((h.speed_bonus() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn speed_bonus_zero_when_disabled() {
        let mut h = Hound::new(0.1, 0.5);
        h.lock_on();
        h.tick(5.0);
        h.enabled = false;
        assert!((h.speed_bonus()).abs() < 1e-5);
    }

    #[test]
    fn effective_speed_scales_while_hunting() {
        let mut h = Hound::new(0.1, 1.0);
        h.lock_on();
        h.tick(2.0); // bonus = 0.1 * 2 = 0.2 → speed = 100 * 1.2 = 120
        assert!((h.effective_speed(100.0) - 120.0).abs() < 1e-3);
    }

    #[test]
    fn effective_speed_base_when_not_hunting() {
        let h = Hound::new(0.1, 0.5);
        assert!((h.effective_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn escalation_fraction_at_half() {
        let mut h = Hound::new(0.1, 1.0);
        h.lock_on();
        h.tick(5.0); // bonus = 0.5, max = 1.0 → fraction = 0.5
        assert!((h.escalation_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn escalation_fraction_zero_when_max_is_zero() {
        let mut h = Hound::new(0.1, 0.0);
        h.lock_on();
        h.tick(10.0);
        assert!((h.escalation_fraction()).abs() < 1e-5);
    }

    #[test]
    fn escalation_fraction_one_at_cap() {
        let mut h = Hound::new(0.1, 0.5);
        h.lock_on();
        h.tick(100.0);
        assert!((h.escalation_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn can_re_lock_after_losing_target() {
        let mut h = Hound::new(0.1, 0.5);
        h.lock_on();
        h.tick(3.0);
        h.lose_target();
        h.tick(0.016);
        h.lock_on();
        assert!(h.is_hunting());
        assert!(h.just_locked);
        assert_eq!(h.chase_timer, 0.0);
    }
}
