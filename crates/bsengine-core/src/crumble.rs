use bevy_ecs::prelude::Component;

/// Structural-decay debuff that linearly degrades an entity's defense
/// from full effectiveness down to `min_fraction` over the debuff duration.
///
/// Unlike `Weaken` (flat penalty from the start) or `ShieldBreak` (only
/// targets shields), Crumble starts at 1.0 and decays — armour/defenses
/// appear to literally crumble away. Defense systems multiply incoming
/// damage reduction by `defense_multiplier()`:
///
///   `defense_multiplier = min_fraction + (1 - min_fraction) * remaining_fraction`
///
/// When the timer reaches zero, `defense_multiplier` is `min_fraction`
/// (worst state) and `just_restored` fires, returning the entity to full
/// defense on the next `tick` call.
///
/// `apply(duration)` uses high-watermark.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Crumble {
    pub duration: f32,
    pub timer: f32,
    /// Minimum defense multiplier reached at full decay [0.0, 1.0].
    pub min_fraction: f32,
    pub just_crumbled: bool,
    pub just_restored: bool,
    pub enabled: bool,
}

impl Crumble {
    pub fn new(min_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            min_fraction: min_fraction.clamp(0.0, 1.0),
            just_crumbled: false,
            just_restored: false,
            enabled: true,
        }
    }

    /// Apply or extend the crumble for `duration` seconds. High-watermark:
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
                self.just_crumbled = true;
            }
        }
    }

    /// Restore defenses immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_restored = true;
        }
    }

    /// Advance the timer; sets `just_restored` when the debuff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_crumbled = false;
        self.just_restored = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_restored = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Current defense effectiveness [min_fraction, 1.0].
    /// Starts at 1.0 when first applied and decays toward `min_fraction`.
    /// Returns 1.0 when inactive.
    pub fn defense_multiplier(&self) -> f32 {
        if !self.is_active() {
            return 1.0;
        }
        let t = self.remaining_fraction();
        self.min_fraction + (1.0 - self.min_fraction) * t
    }

    /// Fraction of the duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Crumble {
    fn default() -> Self {
        Self::new(0.2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_crumble() {
        let mut c = Crumble::new(0.2);
        c.apply(4.0);
        assert!(c.is_active());
        assert!(c.just_crumbled);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut c = Crumble::new(0.2);
        c.apply(2.0);
        c.tick(0.016);
        c.apply(6.0);
        assert!((c.timer - 6.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut c = Crumble::new(0.2);
        c.apply(6.0);
        c.apply(2.0);
        assert!((c.timer - 6.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_crumble() {
        let mut c = Crumble::new(0.2);
        c.apply(1.0);
        c.tick(1.1);
        assert!(!c.is_active());
        assert!(c.just_restored);
    }

    #[test]
    fn clear_ends_early() {
        let mut c = Crumble::new(0.2);
        c.apply(5.0);
        c.clear();
        assert!(!c.is_active());
        assert!(c.just_restored);
    }

    #[test]
    fn defense_multiplier_at_start_is_one() {
        let mut c = Crumble::new(0.2);
        c.apply(4.0);
        // Immediately after apply: timer == duration, remaining_fraction == 1.0
        assert!((c.defense_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn defense_multiplier_decays_to_min_fraction() {
        let mut c = Crumble::new(0.2);
        c.apply(2.0);
        c.tick(2.0 - f32::EPSILON * 100.0); // nearly expired
        let m = c.defense_multiplier();
        assert!(m < 0.25); // approaching min_fraction
    }

    #[test]
    fn defense_multiplier_at_half_duration() {
        let mut c = Crumble::new(0.0);
        c.apply(2.0);
        c.tick(1.0); // half elapsed → remaining_fraction = 0.5
                     // min_fraction=0 → multiplier = 0 + (1-0)*0.5 = 0.5
        assert!((c.defense_multiplier() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn defense_multiplier_when_inactive() {
        let c = Crumble::new(0.2);
        assert!((c.defense_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut c = Crumble::new(0.2);
        c.apply(2.0);
        c.tick(1.0);
        assert!((c.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut c = Crumble::new(0.2);
        c.enabled = false;
        c.apply(5.0);
        assert!(!c.is_active());
    }

    #[test]
    fn tick_clears_just_crumbled() {
        let mut c = Crumble::new(0.2);
        c.apply(3.0);
        c.tick(0.016);
        assert!(!c.just_crumbled);
    }
}
