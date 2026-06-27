use bevy_ecs::prelude::Component;

/// Static commitment bonus: entity gains a damage boost after holding position
/// for at least `min_hold` seconds. The movement system calls `commit(dt)`
/// each frame when the entity is stationary and `reposition()` the moment it
/// moves, keeping the `hold_timer` accurate.
///
/// `commit(dt)` increments `hold_timer` and, on the first frame `hold_timer`
/// reaches `min_hold`, sets `active = true` and fires `just_staked`. It is a
/// no-op when disabled.
///
/// `reposition()` resets `hold_timer` to 0, deactivates the stance, and fires
/// `just_broke` if the entity was staked. No-op if not already active and
/// timer is already zero.
///
/// `tick()` clears one-frame flags each frame (call after reading them).
///
/// `effective_damage(base)` returns `base * (1 + damage_bonus)` while staked
/// and enabled; returns `base` otherwise.
///
/// Distinct from `Stagger` (reeling state after taking a hit), `Stalk`
/// (silent mobile approach), and `Pin` (entity is externally immobilised):
/// Stake is a **voluntary static commitment** — the entity sacrifices mobility
/// willingly to earn a damage bonus for holding the same position.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Stake {
    /// Whether the entity has been stationary long enough to earn the bonus.
    pub active: bool,
    /// Accumulated time the entity has been stationary this session (seconds).
    pub hold_timer: f32,
    /// Minimum stationary duration before the bonus activates. Clamped ≥ 0.0.
    pub min_hold: f32,
    /// Damage bonus fraction while staked. Clamped ≥ 0.0.
    pub damage_bonus: f32,
    pub just_staked: bool,
    pub just_broke: bool,
    pub enabled: bool,
}

impl Stake {
    pub fn new(min_hold: f32, damage_bonus: f32) -> Self {
        Self {
            active: false,
            hold_timer: 0.0,
            min_hold: min_hold.max(0.0),
            damage_bonus: damage_bonus.max(0.0),
            just_staked: false,
            just_broke: false,
            enabled: true,
        }
    }

    /// Advance the stationary timer by `dt`. Activates the bonus and fires
    /// `just_staked` on the frame `hold_timer` first reaches `min_hold`.
    /// No-op when disabled.
    pub fn commit(&mut self, dt: f32) {
        if !self.enabled {
            return;
        }
        let was_active = self.active;
        self.hold_timer += dt;
        if !was_active && self.hold_timer >= self.min_hold {
            self.active = true;
            self.just_staked = true;
        }
    }

    /// Entity moved: reset the hold timer and deactivate the stance. Fires
    /// `just_broke` if the entity was staked. No-op if already idle (timer
    /// is zero and not active).
    pub fn reposition(&mut self) {
        if self.hold_timer <= 0.0 && !self.active {
            return;
        }
        if self.active {
            self.just_broke = true;
        }
        self.hold_timer = 0.0;
        self.active = false;
    }

    /// Clear one-frame flags. Call once per game tick after reading them.
    pub fn tick(&mut self) {
        self.just_staked = false;
        self.just_broke = false;
    }

    /// `true` while staked and enabled.
    pub fn is_staked(&self) -> bool {
        self.active && self.enabled
    }

    /// Outgoing damage with stake bonus applied.
    /// Returns `base * (1 + damage_bonus)` while staked; `base` otherwise.
    pub fn effective_damage(&self, base: f32) -> f32 {
        if self.is_staked() {
            base * (1.0 + self.damage_bonus)
        } else {
            base
        }
    }

    /// How far into the hold window the entity is [0.0 = just arrived,
    /// 1.0 = threshold reached or exceeded]. Returns 0.0 when `min_hold` is 0.
    pub fn hold_fraction(&self) -> f32 {
        if self.min_hold <= 0.0 {
            return 0.0;
        }
        (self.hold_timer / self.min_hold).min(1.0)
    }
}

impl Default for Stake {
    fn default() -> Self {
        Self::new(1.5, 0.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_not_staked() {
        let s = Stake::new(1.5, 0.4);
        assert!(!s.is_staked());
        assert_eq!(s.hold_timer, 0.0);
    }

    #[test]
    fn commit_accumulates_timer() {
        let mut s = Stake::new(2.0, 0.4);
        s.commit(0.5);
        assert!((s.hold_timer - 0.5).abs() < 1e-5);
        assert!(!s.is_staked());
    }

    #[test]
    fn commit_activates_on_threshold() {
        let mut s = Stake::new(1.0, 0.4);
        s.commit(1.0);
        assert!(s.is_staked());
        assert!(s.just_staked);
    }

    #[test]
    fn commit_activates_when_exceeding_threshold() {
        let mut s = Stake::new(1.0, 0.4);
        s.commit(0.7);
        s.commit(0.7); // crosses threshold on second call
        assert!(s.is_staked());
        assert!(s.just_staked);
    }

    #[test]
    fn commit_no_repeat_just_staked() {
        let mut s = Stake::new(1.0, 0.4);
        s.commit(1.0); // activates
        s.tick();
        s.commit(0.5); // already active
        assert!(!s.just_staked);
    }

    #[test]
    fn commit_no_op_when_disabled() {
        let mut s = Stake::new(1.0, 0.4);
        s.enabled = false;
        s.commit(2.0);
        assert!(!s.active);
        assert_eq!(s.hold_timer, 0.0);
    }

    #[test]
    fn reposition_resets_timer_and_deactivates() {
        let mut s = Stake::new(1.0, 0.4);
        s.commit(1.5); // staked
        s.reposition();
        assert!(!s.is_staked());
        assert_eq!(s.hold_timer, 0.0);
        assert!(s.just_broke);
    }

    #[test]
    fn reposition_no_op_when_idle() {
        let mut s = Stake::new(1.0, 0.4);
        s.reposition();
        assert!(!s.just_broke);
    }

    #[test]
    fn reposition_before_threshold_no_just_broke() {
        let mut s = Stake::new(2.0, 0.4);
        s.commit(0.5); // partial hold, not yet staked
        s.reposition();
        assert!(!s.just_broke);
        assert_eq!(s.hold_timer, 0.0);
    }

    #[test]
    fn tick_clears_just_staked() {
        let mut s = Stake::new(1.0, 0.4);
        s.commit(1.0);
        s.tick();
        assert!(!s.just_staked);
    }

    #[test]
    fn tick_clears_just_broke() {
        let mut s = Stake::new(1.0, 0.4);
        s.commit(1.0);
        s.reposition();
        s.tick();
        assert!(!s.just_broke);
    }

    #[test]
    fn is_staked_false_when_disabled() {
        let mut s = Stake::new(1.0, 0.4);
        s.commit(2.0); // meets threshold
        s.enabled = false;
        assert!(!s.is_staked());
    }

    #[test]
    fn effective_damage_applies_bonus() {
        let mut s = Stake::new(1.0, 0.5);
        s.commit(1.0);
        // 100 * (1 + 0.5) = 150
        assert!((s.effective_damage(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_damage_base_when_not_staked() {
        let s = Stake::new(1.0, 0.5);
        assert!((s.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_damage_base_when_disabled() {
        let mut s = Stake::new(1.0, 0.5);
        s.commit(2.0);
        s.enabled = false;
        assert!((s.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn hold_fraction_at_half() {
        let mut s = Stake::new(2.0, 0.4);
        s.commit(1.0);
        assert!((s.hold_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn hold_fraction_caps_at_one() {
        let mut s = Stake::new(1.0, 0.4);
        s.commit(3.0);
        assert!((s.hold_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn hold_fraction_zero_when_fresh() {
        let s = Stake::new(2.0, 0.4);
        assert!((s.hold_fraction()).abs() < 1e-5);
    }

    #[test]
    fn hold_fraction_zero_when_min_hold_zero() {
        let s = Stake::new(0.0, 0.4);
        assert!((s.hold_fraction()).abs() < 1e-5);
    }

    #[test]
    fn min_hold_clamped_non_negative() {
        let s = Stake::new(-1.0, 0.4);
        assert_eq!(s.min_hold, 0.0);
    }

    #[test]
    fn damage_bonus_clamped_non_negative() {
        let s = Stake::new(1.5, -0.4);
        assert_eq!(s.damage_bonus, 0.0);
    }

    #[test]
    fn can_re_stake_after_reposition() {
        let mut s = Stake::new(1.0, 0.4);
        s.commit(1.5); // stake
        s.reposition();
        s.tick();
        s.commit(1.0); // stake again
        assert!(s.is_staked());
        assert!(s.just_staked);
    }

    #[test]
    fn zero_min_hold_stakes_immediately() {
        let mut s = Stake::new(0.0, 0.4);
        s.commit(0.001);
        assert!(s.is_staked());
    }
}
