use bevy_ecs::prelude::Component;

/// Overwhelming-force buff that temporarily lets the entity bypass a fraction
/// of the target's armor or damage reduction.
///
/// While active, `armor_penetration` [0.0, 1.0] represents the fraction of
/// the target's armor ignored. The damage pipeline calls
/// `effective_armor(target_armor)` to get the reduced armor value before
/// applying its damage formula.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_faded` on expiry. `clear()` removes the buff immediately.
///
/// Distinct from `Amplify` (multiplies own output power) and `Expose` (debuff
/// on the target reducing its armor): Overpower is a self-buff that lets the
/// entity punch through existing armor without altering the target's component.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Overpower {
    pub duration: f32,
    pub timer: f32,
    /// Fraction [0.0, 1.0] of the target's armor ignored while overpowered.
    pub armor_penetration: f32,
    pub just_overpowered: bool,
    pub just_faded: bool,
    pub enabled: bool,
}

impl Overpower {
    pub fn new(armor_penetration: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            armor_penetration: armor_penetration.clamp(0.0, 1.0),
            just_overpowered: false,
            just_faded: false,
            enabled: true,
        }
    }

    /// Apply or extend the overpower for `duration` seconds. High-watermark:
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
                self.just_overpowered = true;
            }
        }
    }

    /// Remove the buff immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_faded = true;
        }
    }

    /// Advance the timer; sets `just_faded` when the buff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_overpowered = false;
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

    /// Effective armor value after applying penetration.
    /// Returns `target_armor * (1.0 - armor_penetration)` while active,
    /// `target_armor` otherwise.
    pub fn effective_armor(&self, target_armor: f32) -> f32 {
        if self.is_active() {
            target_armor * (1.0 - self.armor_penetration)
        } else {
            target_armor
        }
    }

    /// Fraction of the buff duration remaining [1.0 = just applied, 0.0 = faded].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Overpower {
    fn default() -> Self {
        Self::new(0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_overpower() {
        let mut o = Overpower::new(0.5);
        o.apply(3.0);
        assert!(o.is_active());
        assert!(o.just_overpowered);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut o = Overpower::new(0.5);
        o.apply(2.0);
        o.tick(0.016);
        o.apply(5.0);
        assert!((o.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut o = Overpower::new(0.5);
        o.apply(5.0);
        o.apply(2.0);
        assert!((o.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_overpower() {
        let mut o = Overpower::new(0.5);
        o.apply(1.0);
        o.tick(1.1);
        assert!(!o.is_active());
        assert!(o.just_faded);
    }

    #[test]
    fn clear_ends_early() {
        let mut o = Overpower::new(0.5);
        o.apply(5.0);
        o.clear();
        assert!(!o.is_active());
        assert!(o.just_faded);
    }

    #[test]
    fn effective_armor_while_active() {
        let mut o = Overpower::new(0.4);
        o.apply(3.0);
        let armor = o.effective_armor(100.0);
        assert!((armor - 60.0).abs() < 1e-3); // 100 * (1 - 0.4)
    }

    #[test]
    fn effective_armor_when_inactive() {
        let o = Overpower::new(0.5);
        assert!((o.effective_armor(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut o = Overpower::new(0.5);
        o.apply(2.0);
        o.tick(1.0);
        assert!((o.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut o = Overpower::new(0.5);
        o.enabled = false;
        o.apply(5.0);
        assert!(!o.is_active());
    }

    #[test]
    fn tick_clears_just_overpowered() {
        let mut o = Overpower::new(0.5);
        o.apply(3.0);
        o.tick(0.016);
        assert!(!o.just_overpowered);
    }

    #[test]
    fn armor_penetration_clamped_to_one() {
        let o = Overpower::new(1.5);
        assert!((o.armor_penetration - 1.0).abs() < 1e-5);
    }
}
