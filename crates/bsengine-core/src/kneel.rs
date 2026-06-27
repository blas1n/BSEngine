use bevy_ecs::prelude::Component;

/// Forced-kneel debuff that locks the entity at ground level and restricts
/// movement while still allowing horizontal shuffle and most combat actions.
///
/// While kneeling, movement systems should apply `effective_speed(base)` as the
/// entity's movement speed; vertical movement (jump, fly, stand) is blocked by
/// convention — the caller is responsible for enforcing this constraint.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_risen` when the entity regains full posture.
///
/// Distinct from `Prone` (lying flat — both vertical and horizontal movement
/// blocked), `Root` (no movement at all), and `Stagger` (brief stun that
/// interrupts actions): Kneel is a forced half-kneeled posture — the entity
/// can still shuffle horizontally at reduced speed and use most abilities, but
/// cannot jump or take vertical movement actions.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Kneel {
    pub duration: f32,
    pub timer: f32,
    /// Fraction of base movement speed remaining while kneeling. Clamped to [0.0, 1.0].
    pub speed_fraction: f32,
    pub just_kneeled: bool,
    pub just_risen: bool,
    pub enabled: bool,
}

impl Kneel {
    pub fn new(speed_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            speed_fraction: speed_fraction.clamp(0.0, 1.0),
            just_kneeled: false,
            just_risen: false,
            enabled: true,
        }
    }

    /// Apply or extend the kneel for `duration` seconds. High-watermark: only
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
                self.just_kneeled = true;
            }
        }
    }

    /// Remove the kneel immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_risen = true;
        }
    }

    /// Advance the timer; sets `just_risen` when the debuff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_kneeled = false;
        self.just_risen = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_risen = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective movement speed while kneeling. Returns `base * speed_fraction`
    /// while active, `base` otherwise.
    pub fn effective_speed(&self, base: f32) -> f32 {
        if self.is_active() {
            base * self.speed_fraction
        } else {
            base
        }
    }

    /// Fraction of the kneel duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Kneel {
    fn default() -> Self {
        Self::new(0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_kneel() {
        let mut k = Kneel::new(0.3);
        k.apply(3.0);
        assert!(k.is_active());
        assert!(k.just_kneeled);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut k = Kneel::new(0.3);
        k.apply(2.0);
        k.tick(0.016);
        k.apply(5.0);
        assert!((k.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut k = Kneel::new(0.3);
        k.apply(5.0);
        k.apply(2.0);
        assert!((k.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_kneel() {
        let mut k = Kneel::new(0.3);
        k.apply(1.0);
        k.tick(1.1);
        assert!(!k.is_active());
        assert!(k.just_risen);
    }

    #[test]
    fn clear_ends_early() {
        let mut k = Kneel::new(0.3);
        k.apply(5.0);
        k.clear();
        assert!(!k.is_active());
        assert!(k.just_risen);
    }

    #[test]
    fn effective_speed_while_active() {
        let mut k = Kneel::new(0.3);
        k.apply(3.0);
        assert!((k.effective_speed(10.0) - 3.0).abs() < 1e-4);
    }

    #[test]
    fn effective_speed_when_inactive() {
        let k = Kneel::new(0.3);
        assert!((k.effective_speed(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut k = Kneel::new(0.3);
        k.apply(2.0);
        k.tick(1.0);
        assert!((k.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut k = Kneel::new(0.3);
        k.enabled = false;
        k.apply(5.0);
        assert!(!k.is_active());
    }

    #[test]
    fn tick_clears_just_kneeled() {
        let mut k = Kneel::new(0.3);
        k.apply(3.0);
        k.tick(0.016);
        assert!(!k.just_kneeled);
    }

    #[test]
    fn speed_fraction_clamped() {
        let k = Kneel::new(1.5);
        assert!((k.speed_fraction - 1.0).abs() < 1e-5);
        let k2 = Kneel::new(-0.2);
        assert!((k2.speed_fraction - 0.0).abs() < 1e-5);
    }
}
