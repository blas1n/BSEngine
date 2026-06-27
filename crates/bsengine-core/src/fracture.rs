use bevy_ecs::prelude::Component;

/// Broken-bone debuff that amplifies incoming physical damage and reduces
/// movement speed.
///
/// While fractured, `incoming_damage_multiplier()` returns a value > 1.0 so
/// the damage pipeline deals extra physical damage, and
/// `effective_move_speed(base)` returns `base * (1.0 - move_speed_penalty)`.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_healed` on expiry. `clear()` removes the fracture early (e.g. heal
/// or restorative item).
///
/// Distinct from `Bleed` (ongoing DoT) and `Crush` (armor reduction): Fracture
/// amplifies received damage and penalises mobility, modelling compromised
/// structural integrity.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Fracture {
    pub duration: f32,
    pub timer: f32,
    /// Multiplier applied to incoming physical damage while fractured (>= 1.0).
    pub damage_amplification: f32,
    /// Fraction of base movement speed lost while fractured [0.0, 1.0].
    pub move_speed_penalty: f32,
    pub just_fractured: bool,
    pub just_healed: bool,
    pub enabled: bool,
}

impl Fracture {
    pub fn new(damage_amplification: f32, move_speed_penalty: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            damage_amplification: damage_amplification.max(1.0),
            move_speed_penalty: move_speed_penalty.clamp(0.0, 1.0),
            just_fractured: false,
            just_healed: false,
            enabled: true,
        }
    }

    /// Apply or extend the fracture for `duration` seconds. High-watermark:
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
                self.just_fractured = true;
            }
        }
    }

    /// Heal the fracture immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_healed = true;
        }
    }

    /// Advance the timer; sets `just_healed` when the effect expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_fractured = false;
        self.just_healed = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_healed = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Multiply incoming physical damage by this value.
    /// Returns `damage_amplification` (>= 1.0) while active, `1.0` otherwise.
    pub fn incoming_damage_multiplier(&self) -> f32 {
        if self.is_active() {
            self.damage_amplification
        } else {
            1.0
        }
    }

    /// Effective move speed after applying the fracture penalty.
    /// Returns `base * (1.0 - move_speed_penalty)` while active, `base` otherwise.
    pub fn effective_move_speed(&self, base: f32) -> f32 {
        if self.is_active() {
            base * (1.0 - self.move_speed_penalty)
        } else {
            base
        }
    }

    /// Fraction of the fracture duration remaining [1.0 = just applied, 0.0 = healed].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Fracture {
    fn default() -> Self {
        Self::new(1.3, 0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_fracture() {
        let mut f = Fracture::new(1.3, 0.3);
        f.apply(3.0);
        assert!(f.is_active());
        assert!(f.just_fractured);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut f = Fracture::new(1.3, 0.3);
        f.apply(2.0);
        f.tick(0.016);
        f.apply(5.0);
        assert!((f.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut f = Fracture::new(1.3, 0.3);
        f.apply(5.0);
        f.apply(2.0);
        assert!((f.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_fracture() {
        let mut f = Fracture::new(1.3, 0.3);
        f.apply(1.0);
        f.tick(1.1);
        assert!(!f.is_active());
        assert!(f.just_healed);
    }

    #[test]
    fn clear_heals_early() {
        let mut f = Fracture::new(1.3, 0.3);
        f.apply(5.0);
        f.clear();
        assert!(!f.is_active());
        assert!(f.just_healed);
    }

    #[test]
    fn incoming_damage_multiplier_while_active() {
        let mut f = Fracture::new(1.5, 0.3);
        f.apply(3.0);
        assert!((f.incoming_damage_multiplier() - 1.5).abs() < 1e-5);
    }

    #[test]
    fn incoming_damage_multiplier_when_inactive() {
        let f = Fracture::new(1.5, 0.3);
        assert!((f.incoming_damage_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_move_speed_while_active() {
        let mut f = Fracture::new(1.3, 0.4);
        f.apply(3.0);
        let speed = f.effective_move_speed(10.0);
        assert!((speed - 6.0).abs() < 1e-4); // 10 * (1 - 0.4)
    }

    #[test]
    fn effective_move_speed_when_inactive() {
        let f = Fracture::new(1.3, 0.4);
        let speed = f.effective_move_speed(10.0);
        assert!((speed - 10.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut f = Fracture::new(1.3, 0.3);
        f.apply(2.0);
        f.tick(1.0);
        assert!((f.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut f = Fracture::new(1.3, 0.3);
        f.enabled = false;
        f.apply(5.0);
        assert!(!f.is_active());
    }

    #[test]
    fn tick_clears_just_fractured() {
        let mut f = Fracture::new(1.3, 0.3);
        f.apply(3.0);
        f.tick(0.016);
        assert!(!f.just_fractured);
    }

    #[test]
    fn damage_amplification_clamped_to_min_one() {
        let f = Fracture::new(0.5, 0.3);
        assert!((f.damage_amplification - 1.0).abs() < 1e-5);
    }
}
