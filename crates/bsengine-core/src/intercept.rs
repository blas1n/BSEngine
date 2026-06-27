use bevy_ecs::prelude::Component;

/// Bodyguard stance that positions the entity to take incoming attacks on
/// behalf of nearby allies within `radius` world units.
///
/// While active, the collision/targeting system should redirect attack
/// resolution to this entity when it is within `radius` of an ally being
/// targeted. The interceptor itself takes `effective_damage(d)` per hit —
/// reduced by `damage_reduction` from its own armour/stance.
///
/// `activate(duration)` starts the stance (no-op if already active);
/// `deactivate()` ends it early; `tick(dt)` fires `just_deactivated` on
/// natural expiry.
///
/// Distinct from `Taunt` (forces enemies to target bearer directly) and
/// `Parry` (per-attack deflection window): Intercept is a persistent zone
/// where the bearer physically steps in front of nearby allies' attacks.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Intercept {
    pub duration: f32,
    pub timer: f32,
    /// Radius within which the entity can intercept attacks on allies.
    pub radius: f32,
    /// Fraction of damage the interceptor absorbs less [0.0, 1.0].
    pub damage_reduction: f32,
    pub just_activated: bool,
    pub just_deactivated: bool,
    pub enabled: bool,
}

impl Intercept {
    pub fn new(radius: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            radius: radius.max(0.0),
            damage_reduction: 0.0,
            just_activated: false,
            just_deactivated: false,
            enabled: true,
        }
    }

    pub fn with_damage_reduction(mut self, reduction: f32) -> Self {
        self.damage_reduction = reduction.clamp(0.0, 1.0);
        self
    }

    /// Begin the intercept stance for `duration` seconds. No-op if already
    /// active or disabled.
    pub fn activate(&mut self, duration: f32) {
        if !self.enabled || self.is_active() {
            return;
        }
        self.duration = duration.max(0.0);
        self.timer = self.duration;
        self.just_activated = true;
    }

    /// Drop the stance early.
    pub fn deactivate(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_deactivated = true;
        }
    }

    /// Advance the timer; sets `just_deactivated` when the stance ends.
    pub fn tick(&mut self, dt: f32) {
        self.just_activated = false;
        self.just_deactivated = false;

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

    /// Actual damage dealt to the interceptor for a hit of `raw_damage`.
    /// Applies `damage_reduction` while active; returns `raw_damage` otherwise.
    pub fn effective_damage(&self, raw_damage: f32) -> f32 {
        if self.is_active() {
            raw_damage * (1.0 - self.damage_reduction)
        } else {
            raw_damage
        }
    }

    /// Fraction of the stance duration remaining [1.0 = just activated, 0.0 = ended].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Intercept {
    fn default() -> Self {
        Self::new(3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activate_starts_stance() {
        let mut i = Intercept::new(3.0);
        i.activate(4.0);
        assert!(i.is_active());
        assert!(i.just_activated);
        assert!((i.timer - 4.0).abs() < 1e-5);
    }

    #[test]
    fn activate_no_op_when_already_active() {
        let mut i = Intercept::new(3.0);
        i.activate(4.0);
        i.tick(0.016);
        let before = i.timer;
        i.activate(10.0);
        assert!((i.timer - before).abs() < 1e-4);
    }

    #[test]
    fn deactivate_ends_stance() {
        let mut i = Intercept::new(3.0);
        i.activate(5.0);
        i.deactivate();
        assert!(!i.is_active());
        assert!(i.just_deactivated);
    }

    #[test]
    fn tick_expires_stance() {
        let mut i = Intercept::new(3.0);
        i.activate(1.0);
        i.tick(1.1);
        assert!(!i.is_active());
        assert!(i.just_deactivated);
    }

    #[test]
    fn tick_clears_just_activated() {
        let mut i = Intercept::new(3.0);
        i.activate(3.0);
        i.tick(0.016);
        assert!(!i.just_activated);
    }

    #[test]
    fn effective_damage_with_reduction() {
        let mut i = Intercept::new(3.0).with_damage_reduction(0.25);
        i.activate(5.0);
        assert!((i.effective_damage(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_damage_no_reduction() {
        let mut i = Intercept::new(3.0);
        i.activate(5.0);
        assert!((i.effective_damage(100.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_damage_when_inactive() {
        let i = Intercept::new(3.0).with_damage_reduction(0.5);
        assert!((i.effective_damage(80.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut i = Intercept::new(3.0);
        i.activate(2.0);
        i.tick(1.0);
        assert!((i.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_activate_no_op() {
        let mut i = Intercept::new(3.0);
        i.enabled = false;
        i.activate(5.0);
        assert!(!i.is_active());
    }

    #[test]
    fn reactivate_after_deactivate() {
        let mut i = Intercept::new(3.0);
        i.activate(2.0);
        i.deactivate();
        i.activate(5.0);
        assert!(i.is_active());
        assert!(i.just_activated);
    }
}
