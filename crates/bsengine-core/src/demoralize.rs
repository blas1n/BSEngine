use bevy_ecs::prelude::Component;

/// Morale-breaking debuff that reduces outgoing damage and gives the entity
/// a per-second probability of triggering a flee response.
///
/// While demoralized, `damage_multiplier()` returns `damage_fraction` (< 1.0)
/// for the damage pipeline, and the AI system calls `check_flee(rng, dt)` to
/// decide whether the entity retreats this frame.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_recovered` on expiry. `clear()` removes the debuff early.
///
/// Distinct from `Weaken` (pure damage penalty, no morale component), `Fear`
/// (full panic state, forced flee), and `Confuse` (misfires on actions):
/// Demoralize is a soft debuff that degrades combat performance and nudges the
/// entity toward retreat without fully removing its agency.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Demoralize {
    pub duration: f32,
    pub timer: f32,
    /// Fraction [0.0, 1.0] of outgoing damage dealt while demoralized.
    pub damage_fraction: f32,
    /// Probability per second [0.0, 1.0] of the entity fleeing.
    pub flee_chance: f32,
    pub just_demoralized: bool,
    pub just_recovered: bool,
    pub enabled: bool,
}

impl Demoralize {
    pub fn new(damage_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            damage_fraction: damage_fraction.clamp(0.0, 1.0),
            flee_chance: 0.0,
            just_demoralized: false,
            just_recovered: false,
            enabled: true,
        }
    }

    pub fn with_flee_chance(mut self, chance: f32) -> Self {
        self.flee_chance = chance.clamp(0.0, 1.0);
        self
    }

    /// Apply or extend the demoralize for `duration` seconds. High-watermark:
    /// only replaces the current timer if the new duration is longer.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_demoralized = true;
            }
        }
    }

    /// Remove the demoralize immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_recovered = true;
        }
    }

    /// Advance the timer; sets `just_recovered` when the debuff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_demoralized = false;
        self.just_recovered = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_recovered = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Multiply outgoing damage by this value.
    /// Returns `damage_fraction` while active, `1.0` otherwise.
    pub fn damage_multiplier(&self) -> f32 {
        if self.is_active() {
            self.damage_fraction
        } else {
            1.0
        }
    }

    /// Returns `true` if the entity should flee this frame. `rng_value` is a
    /// uniform random in `[0.0, 1.0)`. Per-frame threshold: `flee_chance * dt`.
    pub fn check_flee(&self, rng_value: f32, dt: f32) -> bool {
        self.is_active() && rng_value < (self.flee_chance * dt)
    }

    /// Fraction of the demoralize duration remaining [1.0 = just applied, 0.0 = recovered].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Demoralize {
    fn default() -> Self {
        Self::new(0.7).with_flee_chance(0.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_demoralize() {
        let mut d = Demoralize::new(0.7);
        d.apply(3.0);
        assert!(d.is_active());
        assert!(d.just_demoralized);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut d = Demoralize::new(0.7);
        d.apply(2.0);
        d.tick(0.016);
        d.apply(5.0);
        assert!((d.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut d = Demoralize::new(0.7);
        d.apply(5.0);
        d.apply(2.0);
        assert!((d.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_demoralize() {
        let mut d = Demoralize::new(0.7);
        d.apply(1.0);
        d.tick(1.1);
        assert!(!d.is_active());
        assert!(d.just_recovered);
    }

    #[test]
    fn clear_ends_early() {
        let mut d = Demoralize::new(0.7);
        d.apply(5.0);
        d.clear();
        assert!(!d.is_active());
        assert!(d.just_recovered);
    }

    #[test]
    fn damage_multiplier_while_active() {
        let mut d = Demoralize::new(0.6);
        d.apply(3.0);
        assert!((d.damage_multiplier() - 0.6).abs() < 1e-5);
    }

    #[test]
    fn damage_multiplier_when_inactive() {
        let d = Demoralize::new(0.6);
        assert!((d.damage_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn check_flee_true_when_rng_below_threshold() {
        let mut d = Demoralize::new(0.7).with_flee_chance(1.0);
        d.apply(5.0);
        // chance=1.0, dt=1.0 → threshold=1.0; rng=0.5 < 1.0 → true
        assert!(d.check_flee(0.5, 1.0));
    }

    #[test]
    fn check_flee_false_when_rng_above_threshold() {
        let mut d = Demoralize::new(0.7).with_flee_chance(0.1);
        d.apply(5.0);
        // chance=0.1, dt=0.016 → threshold=0.0016; rng=0.5 > 0.0016 → false
        assert!(!d.check_flee(0.5, 0.016));
    }

    #[test]
    fn check_flee_false_when_inactive() {
        let d = Demoralize::new(0.7).with_flee_chance(1.0);
        assert!(!d.check_flee(0.0, 1.0));
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut d = Demoralize::new(0.7);
        d.apply(2.0);
        d.tick(1.0);
        assert!((d.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut d = Demoralize::new(0.7);
        d.enabled = false;
        d.apply(5.0);
        assert!(!d.is_active());
    }

    #[test]
    fn tick_clears_just_demoralized() {
        let mut d = Demoralize::new(0.7);
        d.apply(3.0);
        d.tick(0.016);
        assert!(!d.just_demoralized);
    }
}
