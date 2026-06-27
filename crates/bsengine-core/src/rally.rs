use bevy_ecs::prelude::Component;

/// Leadership shout that temporarily boosts nearby allies' speed and damage.
///
/// While `is_rallying()`, ally systems should query all entities with a `Rally`
/// component and apply `effective_speed(base)` / `effective_damage(base)` to
/// any ally within `aura_radius` world units of the rallying entity.
///
/// `call(duration)` begins the rally (high-watermark). `tick(dt)` counts down
/// and sets `just_ended` when the shout fades. `dismiss()` ends it early.
///
/// Distinct from `Haste` (speed boost on self only), `Empower` (damage boost
/// on self only), and `Aura` (permanent passive effect): Rally is a
/// timed leadership shout — a temporary coordinated burst that benefits allies
/// in the field for its duration, then fades.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Rally {
    pub duration: f32,
    pub timer: f32,
    /// Radius within which allies receive the rally bonuses (world units).
    /// Clamped ≥ 0.0.
    pub aura_radius: f32,
    /// Fraction of base speed added to allies while rallying. Clamped ≥ 0.0.
    /// e.g. 0.2 = allies move at 120% speed.
    pub speed_bonus_fraction: f32,
    /// Fraction of base damage added to allies while rallying. Clamped ≥ 0.0.
    /// e.g. 0.15 = allies deal 115% damage.
    pub damage_bonus_fraction: f32,
    pub just_rallied: bool,
    pub just_ended: bool,
    pub enabled: bool,
}

impl Rally {
    pub fn new(aura_radius: f32, speed_bonus_fraction: f32, damage_bonus_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            aura_radius: aura_radius.max(0.0),
            speed_bonus_fraction: speed_bonus_fraction.max(0.0),
            damage_bonus_fraction: damage_bonus_fraction.max(0.0),
            just_rallied: false,
            just_ended: false,
            enabled: true,
        }
    }

    /// Begin or extend the rally for `duration` seconds. High-watermark: only
    /// replaces the current timer if the new duration is longer. No-op when
    /// disabled.
    pub fn call(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }
        if duration > self.timer {
            let was_active = self.is_rallying();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_rallied = true;
            }
        }
    }

    /// End the rally early (e.g., leader is stunned or killed).
    pub fn dismiss(&mut self) {
        if self.is_rallying() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_ended = true;
        }
    }

    /// Advance the timer; sets `just_ended` when the rally expires naturally.
    pub fn tick(&mut self, dt: f32) {
        self.just_rallied = false;
        self.just_ended = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_ended = true;
            }
        }
    }

    pub fn is_rallying(&self) -> bool {
        self.timer > 0.0
    }

    /// Whether an ally at `distance` world units is within the rally aura.
    pub fn in_range(&self, distance: f32) -> bool {
        self.is_rallying() && distance <= self.aura_radius
    }

    /// Effective movement speed for an ally receiving this rally.
    /// Returns `base * (1 + speed_bonus_fraction)` while active and enabled,
    /// `base` otherwise.
    pub fn effective_speed(&self, base: f32) -> f32 {
        if self.is_rallying() && self.enabled {
            base * (1.0 + self.speed_bonus_fraction)
        } else {
            base
        }
    }

    /// Effective outgoing damage for an ally receiving this rally.
    /// Returns `base * (1 + damage_bonus_fraction)` while active and enabled,
    /// `base` otherwise.
    pub fn effective_damage(&self, base: f32) -> f32 {
        if self.is_rallying() && self.enabled {
            base * (1.0 + self.damage_bonus_fraction)
        } else {
            base
        }
    }

    /// Fraction of the rally duration remaining [1.0 = just called, 0.0 = ended].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Rally {
    fn default() -> Self {
        Self::new(15.0, 0.2, 0.15)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_starts_rally() {
        let mut r = Rally::new(15.0, 0.2, 0.15);
        r.call(5.0);
        assert!(r.is_rallying());
        assert!(r.just_rallied);
    }

    #[test]
    fn call_extends_on_longer_duration() {
        let mut r = Rally::new(15.0, 0.2, 0.15);
        r.call(3.0);
        r.tick(0.016);
        r.call(8.0);
        assert!((r.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn call_no_extend_on_shorter_duration() {
        let mut r = Rally::new(15.0, 0.2, 0.15);
        r.call(8.0);
        r.call(3.0);
        assert!((r.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn just_rallied_not_set_on_extend() {
        let mut r = Rally::new(15.0, 0.2, 0.15);
        r.call(3.0);
        r.tick(0.016);
        r.call(8.0);
        assert!(!r.just_rallied);
    }

    #[test]
    fn dismiss_ends_rally() {
        let mut r = Rally::new(15.0, 0.2, 0.15);
        r.call(5.0);
        r.dismiss();
        assert!(!r.is_rallying());
        assert!(r.just_ended);
    }

    #[test]
    fn tick_expires_rally() {
        let mut r = Rally::new(15.0, 0.2, 0.15);
        r.call(1.0);
        r.tick(1.1);
        assert!(!r.is_rallying());
        assert!(r.just_ended);
    }

    #[test]
    fn tick_clears_just_rallied() {
        let mut r = Rally::new(15.0, 0.2, 0.15);
        r.call(5.0);
        r.tick(0.016);
        assert!(!r.just_rallied);
    }

    #[test]
    fn in_range_true_within_radius() {
        let mut r = Rally::new(15.0, 0.2, 0.15);
        r.call(5.0);
        assert!(r.in_range(10.0));
        assert!(r.in_range(15.0));
    }

    #[test]
    fn in_range_false_beyond_radius() {
        let mut r = Rally::new(15.0, 0.2, 0.15);
        r.call(5.0);
        assert!(!r.in_range(16.0));
    }

    #[test]
    fn in_range_false_when_not_rallying() {
        let r = Rally::new(15.0, 0.2, 0.15);
        assert!(!r.in_range(5.0));
    }

    #[test]
    fn effective_speed_boosted_while_rallying() {
        let mut r = Rally::new(15.0, 0.2, 0.15);
        r.call(5.0);
        assert!((r.effective_speed(10.0) - 12.0).abs() < 1e-4); // 10 * 1.2
    }

    #[test]
    fn effective_speed_unaffected_when_not_rallying() {
        let r = Rally::new(15.0, 0.2, 0.15);
        assert!((r.effective_speed(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn effective_damage_boosted_while_rallying() {
        let mut r = Rally::new(15.0, 0.2, 0.15);
        r.call(5.0);
        assert!((r.effective_damage(100.0) - 115.0).abs() < 1e-3); // 100 * 1.15
    }

    #[test]
    fn effective_damage_unaffected_when_not_rallying() {
        let r = Rally::new(15.0, 0.2, 0.15);
        assert!((r.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut r = Rally::new(15.0, 0.2, 0.15);
        r.call(4.0);
        r.tick(2.0);
        assert!((r.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_call_no_op() {
        let mut r = Rally::new(15.0, 0.2, 0.15);
        r.enabled = false;
        r.call(5.0);
        assert!(!r.is_rallying());
    }

    #[test]
    fn disabled_effective_speed_unaffected() {
        let mut r = Rally::new(15.0, 0.2, 0.15);
        r.call(5.0);
        r.enabled = false;
        assert!((r.effective_speed(10.0) - 10.0).abs() < 1e-5);
    }
}
