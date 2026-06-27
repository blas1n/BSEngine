use bevy_ecs::prelude::Component;

/// Reactive defense that gives the entity a per-hit chance to deflect
/// incoming projectiles or attacks, optionally returning a fraction of the
/// damage to the attacker.
///
/// While active, the hit-resolution system calls `check_deflect(rng)` for each
/// incoming hit. On a deflect, apply `reflected_damage(original)` to the
/// attacker instead of the original damage to the defender.
///
/// `activate(duration)` is a no-op if already active or disabled. Call
/// `deactivate()` to end early (e.g. on a counter-hit that breaks the stance).
/// `just_deflected` is set for one tick on the frame a deflect fires so VFX
/// and audio systems can react.
///
/// Distinct from `Parry` (timing-gated active block), `Reflect` (sustained
/// magical mirror), and `Dodge` (full evasion): Deflect is a passive, probabilistic
/// redirection that only partially negates incoming damage.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Deflect {
    pub duration: f32,
    pub timer: f32,
    /// Probability [0.0, 1.0] of deflecting each incoming hit.
    pub chance: f32,
    /// Fraction [0.0, 1.0] of original damage returned to attacker on deflect.
    pub reflected_damage_fraction: f32,
    pub just_activated: bool,
    pub just_deactivated: bool,
    /// Set for one tick when a deflect fires (caller must set this manually).
    pub just_deflected: bool,
    pub enabled: bool,
}

impl Deflect {
    pub fn new(chance: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            chance: chance.clamp(0.0, 1.0),
            reflected_damage_fraction: 0.0,
            just_activated: false,
            just_deactivated: false,
            just_deflected: false,
            enabled: true,
        }
    }

    pub fn with_reflected_damage(mut self, fraction: f32) -> Self {
        self.reflected_damage_fraction = fraction.clamp(0.0, 1.0);
        self
    }

    /// Activate the deflect stance for `duration` seconds. No-op if already
    /// active or disabled.
    pub fn activate(&mut self, duration: f32) {
        if !self.enabled || self.is_active() {
            return;
        }
        self.duration = duration.max(0.0);
        self.timer = self.duration;
        self.just_activated = true;
    }

    /// End the deflect stance early.
    pub fn deactivate(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_deactivated = true;
        }
    }

    /// Advance the timer; sets `just_deactivated` on expiry.
    /// Also clears `just_activated` and `just_deflected` from the previous tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_activated = false;
        self.just_deactivated = false;
        self.just_deflected = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_deactivated = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Returns `true` if this hit is deflected. `rng_value` is a uniform
    /// random in `[0.0, 1.0)`. Returns false if not active.
    pub fn check_deflect(&self, rng_value: f32) -> bool {
        self.is_active() && rng_value < self.chance
    }

    /// Damage to deal to the attacker when a deflect succeeds.
    pub fn reflected_damage(&self, original_damage: f32) -> f32 {
        original_damage * self.reflected_damage_fraction
    }

    /// Fraction of the deflect duration remaining [1.0 = just activated, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Deflect {
    fn default() -> Self {
        Self::new(0.25).with_reflected_damage(0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activate_starts_deflect() {
        let mut d = Deflect::new(0.5);
        d.activate(3.0);
        assert!(d.is_active());
        assert!(d.just_activated);
    }

    #[test]
    fn activate_no_op_when_already_active() {
        let mut d = Deflect::new(0.5);
        d.activate(3.0);
        d.tick(0.016);
        let before = d.timer;
        d.activate(10.0);
        assert!((d.timer - before).abs() < 1e-4);
    }

    #[test]
    fn deactivate_ends_early() {
        let mut d = Deflect::new(0.5);
        d.activate(5.0);
        d.deactivate();
        assert!(!d.is_active());
        assert!(d.just_deactivated);
    }

    #[test]
    fn tick_expires_deflect() {
        let mut d = Deflect::new(0.5);
        d.activate(1.0);
        d.tick(1.1);
        assert!(!d.is_active());
        assert!(d.just_deactivated);
    }

    #[test]
    fn check_deflect_true_when_rng_below_chance() {
        let mut d = Deflect::new(0.8);
        d.activate(5.0);
        assert!(d.check_deflect(0.5)); // 0.5 < 0.8
    }

    #[test]
    fn check_deflect_false_when_rng_above_chance() {
        let mut d = Deflect::new(0.3);
        d.activate(5.0);
        assert!(!d.check_deflect(0.8)); // 0.8 >= 0.3
    }

    #[test]
    fn check_deflect_false_when_inactive() {
        let d = Deflect::new(0.9);
        assert!(!d.check_deflect(0.0));
    }

    #[test]
    fn reflected_damage_fraction_applied() {
        let d = Deflect::new(0.5).with_reflected_damage(0.4);
        assert!((d.reflected_damage(100.0) - 40.0).abs() < 1e-4);
    }

    #[test]
    fn reflected_damage_zero_when_fraction_zero() {
        let d = Deflect::new(0.5);
        assert!((d.reflected_damage(100.0) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut d = Deflect::new(0.5);
        d.activate(2.0);
        d.tick(1.0);
        assert!((d.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_activate_no_op() {
        let mut d = Deflect::new(0.5);
        d.enabled = false;
        d.activate(5.0);
        assert!(!d.is_active());
    }

    #[test]
    fn tick_clears_just_activated() {
        let mut d = Deflect::new(0.5);
        d.activate(5.0);
        d.tick(0.016);
        assert!(!d.just_activated);
    }

    #[test]
    fn tick_clears_just_deflected() {
        let mut d = Deflect::new(0.5);
        d.activate(5.0);
        d.just_deflected = true;
        d.tick(0.016);
        assert!(!d.just_deflected);
    }
}
