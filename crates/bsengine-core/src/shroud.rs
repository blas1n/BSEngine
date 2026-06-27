use bevy_ecs::prelude::Component;

/// Death-prevention veil that intercepts lethal hits and saves the entity at
/// a fraction of their maximum health instead of dying.
///
/// When the damage pipeline would reduce HP to zero or below, it should call
/// `try_absorb()` before applying the final blow. If the shroud has charges
/// and its cooldown has elapsed, `try_absorb()` consumes one charge, sets
/// `just_saved`, and returns `Some(save_health_fraction)` — the health
/// fraction to set. If unavailable, returns `None` (hit proceeds normally).
///
/// `tick(dt)` counts down `cooldown_timer`. After a save, `cooldown_timer` is
/// set to `cooldown` and cannot trigger again until it reaches zero.
///
/// Distinct from `Invincible` (blocks all damage for a duration) and
/// `Barrier` (absorbs flat HP before main health takes hits): Shroud only
/// triggers on the killing blow and leaves the entity alive at low health,
/// rewarding recovery play.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Shroud {
    /// Remaining death-prevention charges.
    pub charges: u32,
    /// HP fraction the entity is set to after a save. Clamped to (0.0, 1.0].
    /// e.g. 0.1 = entity survives at 10% of max HP.
    pub save_health_fraction: f32,
    /// Minimum seconds between consecutive saves (prevents multi-hit instant death).
    pub cooldown: f32,
    pub cooldown_timer: f32,
    pub just_saved: bool,
    pub just_exhausted: bool,
    pub enabled: bool,
}

impl Shroud {
    pub fn new(save_health_fraction: f32, charges: u32) -> Self {
        Self {
            charges,
            save_health_fraction: save_health_fraction.clamp(f32::EPSILON, 1.0),
            cooldown: 0.0,
            cooldown_timer: 0.0,
            just_saved: false,
            just_exhausted: false,
            enabled: true,
        }
    }

    pub fn with_cooldown(mut self, cooldown: f32) -> Self {
        self.cooldown = cooldown.max(0.0);
        self
    }

    /// Attempt to absorb a killing blow. Returns `Some(save_health_fraction)`
    /// if a charge was spent, `None` if the shroud is unavailable.
    pub fn try_absorb(&mut self) -> Option<f32> {
        if !self.enabled || self.charges == 0 || self.cooldown_timer > 0.0 {
            return None;
        }
        self.charges -= 1;
        self.cooldown_timer = self.cooldown;
        self.just_saved = true;
        if self.charges == 0 {
            self.just_exhausted = true;
        }
        Some(self.save_health_fraction)
    }

    /// Advance the cooldown timer.
    pub fn tick(&mut self, dt: f32) {
        self.just_saved = false;
        self.just_exhausted = false;

        if self.cooldown_timer > 0.0 {
            self.cooldown_timer -= dt;
            if self.cooldown_timer < 0.0 {
                self.cooldown_timer = 0.0;
            }
        }
    }

    /// Returns true when the shroud can intercept the next lethal hit.
    pub fn is_ready(&self) -> bool {
        self.enabled && self.charges > 0 && self.cooldown_timer <= 0.0
    }

    /// Fraction of the original charges remaining [0.0, 1.0].
    /// Returns 0.0 when constructed with 0 charges.
    pub fn charges_fraction(&self, max_charges: u32) -> f32 {
        if max_charges == 0 {
            return 0.0;
        }
        (self.charges as f32 / max_charges as f32).clamp(0.0, 1.0)
    }
}

impl Default for Shroud {
    fn default() -> Self {
        Self::new(0.1, 1).with_cooldown(30.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_absorb_saves_on_killing_blow() {
        let mut s = Shroud::new(0.1, 1);
        let result = s.try_absorb();
        assert!(result.is_some());
        assert!((result.unwrap() - 0.1).abs() < 1e-5);
        assert!(s.just_saved);
    }

    #[test]
    fn try_absorb_consumes_charge() {
        let mut s = Shroud::new(0.1, 2);
        s.try_absorb();
        assert_eq!(s.charges, 1);
    }

    #[test]
    fn try_absorb_sets_just_exhausted_on_last_charge() {
        let mut s = Shroud::new(0.1, 1);
        s.try_absorb();
        assert!(s.just_exhausted);
        assert_eq!(s.charges, 0);
    }

    #[test]
    fn try_absorb_fails_when_no_charges() {
        let mut s = Shroud::new(0.1, 0);
        assert!(s.try_absorb().is_none());
        assert!(!s.just_saved);
    }

    #[test]
    fn try_absorb_fails_when_on_cooldown() {
        let mut s = Shroud::new(0.1, 2).with_cooldown(5.0);
        s.try_absorb();
        let result = s.try_absorb();
        assert!(result.is_none());
    }

    #[test]
    fn cooldown_expires_after_tick() {
        let mut s = Shroud::new(0.1, 2).with_cooldown(1.0);
        s.try_absorb();
        s.tick(1.1);
        assert!(s.is_ready());
    }

    #[test]
    fn is_ready_false_when_disabled() {
        let mut s = Shroud::new(0.1, 1);
        s.enabled = false;
        assert!(!s.is_ready());
    }

    #[test]
    fn tick_clears_just_saved() {
        let mut s = Shroud::new(0.1, 1);
        s.try_absorb();
        s.tick(0.016);
        assert!(!s.just_saved);
    }

    #[test]
    fn charges_fraction_at_half() {
        let s = Shroud::new(0.1, 1).with_cooldown(0.0);
        assert!((s.charges_fraction(2) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn try_absorb_no_op_when_disabled() {
        let mut s = Shroud::new(0.1, 3);
        s.enabled = false;
        assert!(s.try_absorb().is_none());
        assert_eq!(s.charges, 3);
    }
}
