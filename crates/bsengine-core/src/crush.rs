use bevy_ecs::prelude::Component;

/// Armor-breaking debuff that strips a fraction of the target's physical damage
/// reduction for its duration.
///
/// While crushed, the damage pipeline should compute effective armor via
/// `effective_armor(base_armor)` = `base_armor * (1 - armor_reduction_fraction)`
/// before applying it to incoming physical damage. This makes the entity take
/// more damage from physical hits.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_recovered` when the crush wears off.
///
/// Distinct from `Cripple` (mobility penalty), `ShieldBreak` (destroys a shield
/// layer), `Corrosion` (damage-over-time armor decay), and `Crumble` (collapses
/// structures): Crush is a timed flat-fraction armor reduction — a "dented
/// plate" debuff applied by heavy blows, siege weapons, or earth magic.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Crush {
    pub duration: f32,
    pub timer: f32,
    /// Fraction of armor stripped while crushed. Clamped to [0.0, 1.0].
    /// e.g. 0.5 = 50% of base armor is removed.
    pub armor_reduction_fraction: f32,
    pub just_crushed: bool,
    pub just_recovered: bool,
    pub enabled: bool,
}

impl Crush {
    pub fn new(armor_reduction_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            armor_reduction_fraction: armor_reduction_fraction.clamp(0.0, 1.0),
            just_crushed: false,
            just_recovered: false,
            enabled: true,
        }
    }

    /// Apply or extend the crush for `duration` seconds. High-watermark: only
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
                self.just_crushed = true;
            }
        }
    }

    /// Remove the crush immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_recovered = true;
        }
    }

    /// Advance the timer; sets `just_recovered` when the debuff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_crushed = false;
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

    /// Effective armor after applying the crush reduction.
    /// Returns `base_armor * (1 - armor_reduction_fraction)` while active,
    /// `base_armor` otherwise.
    pub fn effective_armor(&self, base_armor: f32) -> f32 {
        if self.is_active() {
            base_armor * (1.0 - self.armor_reduction_fraction)
        } else {
            base_armor
        }
    }

    /// Fraction of the crush duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Crush {
    fn default() -> Self {
        Self::new(0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_crush() {
        let mut c = Crush::new(0.5);
        c.apply(3.0);
        assert!(c.is_active());
        assert!(c.just_crushed);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut c = Crush::new(0.5);
        c.apply(2.0);
        c.tick(0.016);
        c.apply(5.0);
        assert!((c.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut c = Crush::new(0.5);
        c.apply(5.0);
        c.apply(2.0);
        assert!((c.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_crush() {
        let mut c = Crush::new(0.5);
        c.apply(1.0);
        c.tick(1.1);
        assert!(!c.is_active());
        assert!(c.just_recovered);
    }

    #[test]
    fn clear_ends_early() {
        let mut c = Crush::new(0.5);
        c.apply(5.0);
        c.clear();
        assert!(!c.is_active());
        assert!(c.just_recovered);
    }

    #[test]
    fn effective_armor_while_active() {
        let mut c = Crush::new(0.5);
        c.apply(3.0);
        assert!((c.effective_armor(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_armor_when_inactive() {
        let c = Crush::new(0.5);
        assert!((c.effective_armor(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_armor_full_strip() {
        let mut c = Crush::new(1.0);
        c.apply(3.0);
        assert!((c.effective_armor(200.0) - 0.0).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut c = Crush::new(0.5);
        c.apply(2.0);
        c.tick(1.0);
        assert!((c.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut c = Crush::new(0.5);
        c.enabled = false;
        c.apply(5.0);
        assert!(!c.is_active());
    }

    #[test]
    fn tick_clears_just_crushed() {
        let mut c = Crush::new(0.5);
        c.apply(3.0);
        c.tick(0.016);
        assert!(!c.just_crushed);
    }

    #[test]
    fn armor_reduction_fraction_clamped() {
        let c = Crush::new(1.5);
        assert!((c.armor_reduction_fraction - 1.0).abs() < 1e-5);
        let c2 = Crush::new(-0.3);
        assert!((c2.armor_reduction_fraction - 0.0).abs() < 1e-5);
    }
}
