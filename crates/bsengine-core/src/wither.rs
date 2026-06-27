use bevy_ecs::prelude::Component;

/// Decay debuff that temporarily lowers the entity's maximum health ceiling,
/// preventing full recovery while withered.
///
/// `effective_max_health(base)` returns `base * (1.0 - max_health_fraction)` —
/// the health system caps the entity's current HP at this value. A `max_health_fraction`
/// of 0.3 reduces a 1000 HP entity's ceiling to 700 HP until the debuff clears.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_recovered` on expiry. `clear()` removes the debuff early.
///
/// Distinct from `Weaken` (outgoing damage reduction) and `Exhaust` (stamina
/// cap): Wither compresses the health ceiling itself, making the entity easier
/// to kill during the window without affecting its damage or speed.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wither {
    pub duration: f32,
    pub timer: f32,
    /// Fraction [0.0, 1.0] subtracted from max health while withered.
    /// e.g. 0.3 = entity's effective max HP is 70% of base.
    pub max_health_fraction: f32,
    pub just_withered: bool,
    pub just_recovered: bool,
    pub enabled: bool,
}

impl Wither {
    pub fn new(max_health_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            max_health_fraction: max_health_fraction.clamp(0.0, 1.0),
            just_withered: false,
            just_recovered: false,
            enabled: true,
        }
    }

    /// Apply or extend the wither for `duration` seconds. High-watermark:
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
                self.just_withered = true;
            }
        }
    }

    /// Remove the wither immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_recovered = true;
        }
    }

    /// Advance the timer; sets `just_recovered` when the debuff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_withered = false;
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

    /// Effective maximum health after applying the wither.
    /// Returns `base * (1.0 - max_health_fraction)` while active, `base` otherwise.
    pub fn effective_max_health(&self, base: f32) -> f32 {
        if self.is_active() {
            base * (1.0 - self.max_health_fraction)
        } else {
            base
        }
    }

    /// Fraction of the wither duration remaining [1.0 = just applied, 0.0 = recovered].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Wither {
    fn default() -> Self {
        Self::new(0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_wither() {
        let mut w = Wither::new(0.3);
        w.apply(3.0);
        assert!(w.is_active());
        assert!(w.just_withered);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut w = Wither::new(0.3);
        w.apply(2.0);
        w.tick(0.016);
        w.apply(5.0);
        assert!((w.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut w = Wither::new(0.3);
        w.apply(5.0);
        w.apply(2.0);
        assert!((w.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_wither() {
        let mut w = Wither::new(0.3);
        w.apply(1.0);
        w.tick(1.1);
        assert!(!w.is_active());
        assert!(w.just_recovered);
    }

    #[test]
    fn clear_ends_early() {
        let mut w = Wither::new(0.3);
        w.apply(5.0);
        w.clear();
        assert!(!w.is_active());
        assert!(w.just_recovered);
    }

    #[test]
    fn effective_max_health_while_active() {
        let mut w = Wither::new(0.3);
        w.apply(3.0);
        let hp = w.effective_max_health(1000.0);
        assert!((hp - 700.0).abs() < 1e-2); // 1000 * (1 - 0.3)
    }

    #[test]
    fn effective_max_health_when_inactive() {
        let w = Wither::new(0.3);
        assert!((w.effective_max_health(1000.0) - 1000.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut w = Wither::new(0.3);
        w.apply(2.0);
        w.tick(1.0);
        assert!((w.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut w = Wither::new(0.3);
        w.enabled = false;
        w.apply(5.0);
        assert!(!w.is_active());
    }

    #[test]
    fn tick_clears_just_withered() {
        let mut w = Wither::new(0.3);
        w.apply(3.0);
        w.tick(0.016);
        assert!(!w.just_withered);
    }

    #[test]
    fn max_health_fraction_clamped_to_one() {
        let w = Wither::new(1.5);
        assert!((w.max_health_fraction - 1.0).abs() < 1e-5);
    }
}
