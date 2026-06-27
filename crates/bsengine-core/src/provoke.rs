use bevy_ecs::prelude::Component;

/// Buff that dramatically increases the bearer's threat within a radius,
/// drawing nearby enemy AI to prioritize this entity as their target.
///
/// While active, the aggro/threat system should multiply this entity's
/// perceived threat by `effective_aggro_multiplier()` for all enemies within
/// `radius` world units. `activate(duration)` starts the effect; `tick(dt)`
/// counts down and sets `just_expired` when it ends.
///
/// Distinct from `Taunt` (a debuff placed on an attacker forcing it to
/// attack a specific target) and `Mark` (a tracker/visibility tool):
/// Provoke is a self-buff that amplifies the bearer's threat value in the
/// enemy AI priority queue.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Provoke {
    pub duration: f32,
    pub timer: f32,
    /// Multiplier applied to this entity's threat score for nearby enemies.
    pub aggro_multiplier: f32,
    /// World-unit radius within which enemies detect the provoke.
    pub radius: f32,
    pub just_provoked: bool,
    pub just_expired: bool,
    pub enabled: bool,
}

impl Provoke {
    pub fn new(aggro_multiplier: f32, radius: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            aggro_multiplier: aggro_multiplier.max(1.0),
            radius: radius.max(0.0),
            just_provoked: false,
            just_expired: false,
            enabled: true,
        }
    }

    /// Activate the provoke for `duration` seconds. No-op if already active
    /// or disabled; call `deactivate()` first to restart.
    pub fn activate(&mut self, duration: f32) {
        if !self.enabled || self.is_active() {
            return;
        }
        self.duration = duration.max(0.0);
        self.timer = self.duration;
        self.just_provoked = true;
    }

    /// End the provoke immediately.
    pub fn deactivate(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_expired = true;
        }
    }

    /// Advance the timer; sets `just_expired` when the effect ends.
    pub fn tick(&mut self, dt: f32) {
        self.just_provoked = false;
        self.just_expired = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_expired = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Threat multiplier to apply to this entity. Returns `aggro_multiplier`
    /// while active, `1.0` otherwise.
    pub fn effective_aggro_multiplier(&self) -> f32 {
        if self.is_active() {
            self.aggro_multiplier
        } else {
            1.0
        }
    }

    /// Fraction of the provoke duration remaining [1.0 = just activated, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Provoke {
    fn default() -> Self {
        Self::new(3.0, 15.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activate_starts_provoke() {
        let mut p = Provoke::new(3.0, 15.0);
        p.activate(4.0);
        assert!(p.is_active());
        assert!(p.just_provoked);
        assert!((p.timer - 4.0).abs() < 1e-5);
    }

    #[test]
    fn activate_no_op_when_already_active() {
        let mut p = Provoke::new(3.0, 15.0);
        p.activate(4.0);
        p.tick(0.016);
        let before = p.timer;
        p.activate(10.0);
        assert!((p.timer - before).abs() < 1e-4);
    }

    #[test]
    fn deactivate_ends_provoke() {
        let mut p = Provoke::new(3.0, 15.0);
        p.activate(5.0);
        p.deactivate();
        assert!(!p.is_active());
        assert!(p.just_expired);
    }

    #[test]
    fn tick_expires_provoke() {
        let mut p = Provoke::new(3.0, 15.0);
        p.activate(1.0);
        p.tick(1.1);
        assert!(!p.is_active());
        assert!(p.just_expired);
    }

    #[test]
    fn tick_clears_just_provoked() {
        let mut p = Provoke::new(3.0, 15.0);
        p.activate(3.0);
        p.tick(0.016);
        assert!(!p.just_provoked);
    }

    #[test]
    fn effective_aggro_multiplier_while_active() {
        let mut p = Provoke::new(4.0, 15.0);
        p.activate(3.0);
        assert!((p.effective_aggro_multiplier() - 4.0).abs() < 1e-5);
    }

    #[test]
    fn effective_aggro_multiplier_when_inactive() {
        let p = Provoke::new(4.0, 15.0);
        assert!((p.effective_aggro_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn aggro_multiplier_clamped_to_one() {
        let p = Provoke::new(0.5, 15.0);
        assert!((p.aggro_multiplier - 1.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut p = Provoke::new(3.0, 15.0);
        p.activate(2.0);
        p.tick(1.0);
        assert!((p.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_activate_no_op() {
        let mut p = Provoke::new(3.0, 15.0);
        p.enabled = false;
        p.activate(5.0);
        assert!(!p.is_active());
    }

    #[test]
    fn reactivate_after_deactivate() {
        let mut p = Provoke::new(3.0, 15.0);
        p.activate(2.0);
        p.deactivate();
        p.activate(5.0);
        assert!(p.is_active());
        assert!(p.just_provoked);
    }
}
