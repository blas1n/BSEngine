use bevy_ecs::prelude::Component;

/// Root CC that immobilizes an entity's movement while leaving attacking and
/// casting fully available (unlike `Stun` which locks everything).
///
/// Common in MOBAs and ARPGs: a snare/root effect binds the target in place
/// but does not interrupt active spells or auto-attacks. The physics/movement
/// system should skip velocity integration (or zero it) while `is_active()`.
///
/// `apply(duration)` uses high-watermark; `tick(dt)` counts down and sets
/// `just_unentangled` when the effect expires.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Entangle {
    pub duration: f32,
    pub timer: f32,
    pub just_entangled: bool,
    pub just_unentangled: bool,
    pub enabled: bool,
}

impl Entangle {
    pub fn new() -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            just_entangled: false,
            just_unentangled: false,
            enabled: true,
        }
    }

    /// Apply or extend an entangle of `duration` seconds. High-watermark: only
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
                self.just_entangled = true;
            }
        }
    }

    /// Remove the effect immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_unentangled = true;
        }
    }

    /// Advance the timer; sets `just_unentangled` when the effect expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_entangled = false;
        self.just_unentangled = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_unentangled = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Fraction of the entangle duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Entangle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_entangle() {
        let mut e = Entangle::new();
        e.apply(3.0);
        assert!(e.is_active());
        assert!(e.just_entangled);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut e = Entangle::new();
        e.apply(2.0);
        e.tick(0.016);
        e.apply(5.0);
        assert!((e.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut e = Entangle::new();
        e.apply(5.0);
        e.apply(2.0);
        assert!((e.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_entangle() {
        let mut e = Entangle::new();
        e.apply(1.0);
        e.tick(1.1);
        assert!(!e.is_active());
        assert!(e.just_unentangled);
    }

    #[test]
    fn clear_ends_entangle_early() {
        let mut e = Entangle::new();
        e.apply(5.0);
        e.clear();
        assert!(!e.is_active());
        assert!(e.just_unentangled);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut e = Entangle::new();
        e.apply(2.0);
        e.tick(1.0);
        let frac = e.remaining_fraction();
        assert!((frac - 0.5).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_entangled() {
        let mut e = Entangle::new();
        e.apply(3.0);
        e.tick(0.016);
        assert!(!e.just_entangled);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut e = Entangle::new();
        e.enabled = false;
        e.apply(5.0);
        assert!(!e.is_active());
    }

    #[test]
    fn remaining_fraction_zero_when_inactive() {
        let e = Entangle::new();
        assert!((e.remaining_fraction()).abs() < 1e-5);
    }
}
