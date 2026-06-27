use bevy_ecs::prelude::Component;

/// Fortune/luck buff that multiplies drop rates and experience gain for its
/// duration.
///
/// While active, reward systems should scale their outputs by the corresponding
/// multipliers: `effective_drop_rate(base)` for loot drops and
/// `effective_experience(base)` for XP rewards. Both return `base` when the
/// boon is inactive.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_faded` when the boon expires. `clear()` removes it early.
///
/// Distinct from `Boost` (raw stat bonuses), `Amplify` (ability potency
/// multiplier), and `Empower` (potency buff with duration): Boon is
/// specifically a fortune/reward multiplier — it does not affect combat stats,
/// only post-combat and exploration payouts.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Boon {
    pub duration: f32,
    pub timer: f32,
    /// Multiplier applied to item drop rates. e.g. 1.5 = 50% more drops.
    pub drop_rate_bonus: f32,
    /// Multiplier applied to experience point rewards. e.g. 2.0 = double XP.
    pub experience_bonus: f32,
    pub just_blessed: bool,
    pub just_faded: bool,
    pub enabled: bool,
}

impl Boon {
    pub fn new(drop_rate_bonus: f32, experience_bonus: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            drop_rate_bonus: drop_rate_bonus.max(1.0),
            experience_bonus: experience_bonus.max(1.0),
            just_blessed: false,
            just_faded: false,
            enabled: true,
        }
    }

    /// Apply or extend the boon for `duration` seconds. High-watermark: only
    /// replaces the current timer if the new duration is longer.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_blessed = true;
            }
        }
    }

    /// Remove the boon immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_faded = true;
        }
    }

    /// Advance the timer; sets `just_faded` when the boon expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_blessed = false;
        self.just_faded = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_faded = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective drop rate after applying the bonus multiplier.
    /// Returns `base * drop_rate_bonus` while active, `base` otherwise.
    pub fn effective_drop_rate(&self, base: f32) -> f32 {
        if self.is_active() {
            base * self.drop_rate_bonus
        } else {
            base
        }
    }

    /// Effective experience reward after applying the bonus multiplier.
    /// Returns `base * experience_bonus` while active, `base` otherwise.
    pub fn effective_experience(&self, base: f32) -> f32 {
        if self.is_active() {
            base * self.experience_bonus
        } else {
            base
        }
    }

    /// Fraction of the boon duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Boon {
    fn default() -> Self {
        Self::new(1.5, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_boon() {
        let mut b = Boon::new(1.5, 2.0);
        b.apply(3.0);
        assert!(b.is_active());
        assert!(b.just_blessed);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut b = Boon::new(1.5, 2.0);
        b.apply(2.0);
        b.tick(0.016);
        b.apply(5.0);
        assert!((b.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut b = Boon::new(1.5, 2.0);
        b.apply(5.0);
        b.apply(2.0);
        assert!((b.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_boon() {
        let mut b = Boon::new(1.5, 2.0);
        b.apply(1.0);
        b.tick(1.1);
        assert!(!b.is_active());
        assert!(b.just_faded);
    }

    #[test]
    fn clear_ends_early() {
        let mut b = Boon::new(1.5, 2.0);
        b.apply(5.0);
        b.clear();
        assert!(!b.is_active());
        assert!(b.just_faded);
    }

    #[test]
    fn effective_drop_rate_while_active() {
        let mut b = Boon::new(2.0, 1.0);
        b.apply(3.0);
        assert!((b.effective_drop_rate(0.1) - 0.2).abs() < 1e-5);
    }

    #[test]
    fn effective_drop_rate_when_inactive() {
        let b = Boon::new(2.0, 1.0);
        assert!((b.effective_drop_rate(0.1) - 0.1).abs() < 1e-5);
    }

    #[test]
    fn effective_experience_while_active() {
        let mut b = Boon::new(1.0, 3.0);
        b.apply(3.0);
        assert!((b.effective_experience(100.0) - 300.0).abs() < 1e-3);
    }

    #[test]
    fn effective_experience_when_inactive() {
        let b = Boon::new(1.0, 3.0);
        assert!((b.effective_experience(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut b = Boon::new(1.5, 2.0);
        b.apply(2.0);
        b.tick(1.0);
        assert!((b.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut b = Boon::new(1.5, 2.0);
        b.enabled = false;
        b.apply(5.0);
        assert!(!b.is_active());
    }

    #[test]
    fn tick_clears_just_blessed() {
        let mut b = Boon::new(1.5, 2.0);
        b.apply(3.0);
        b.tick(0.016);
        assert!(!b.just_blessed);
    }

    #[test]
    fn drop_rate_bonus_clamped_to_min_one() {
        let b = Boon::new(0.5, 0.1); // both below 1.0 → clamped to 1.0
        assert!((b.drop_rate_bonus - 1.0).abs() < 1e-5);
        assert!((b.experience_bonus - 1.0).abs() < 1e-5);
    }
}
