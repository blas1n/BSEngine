use bevy_ecs::prelude::Component;

/// Physical size amplification: the entity swells in scale, simultaneously
/// gaining bonus melee damage and paying a speed penalty. Collision/hitbox
/// systems should apply `scale_multiplier` to the entity's size while
/// `is_enlarged()` and enabled.
///
/// `grow(duration)` starts or extends the effect (high-watermark); sets
/// `just_enlarged` on inactive → active. `shrink()` ends it early. `tick(dt)`
/// counts down and sets `just_shrunken` on expiry.
///
/// Distinct from `Boost` (generic stat multiplier), `Empower` (damage only,
/// no size change or speed penalty), and `Haste` (speed bonus, no size change):
/// Enlarge is a **physical growth trade-off** — bigger, harder hitting, and
/// slower all at once.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Enlarge {
    pub duration: f32,
    pub timer: f32,
    /// Scale factor applied to the entity's collision/visual size. Clamped ≥ 1.0.
    pub scale_multiplier: f32,
    /// Fraction of base melee damage added while enlarged. Clamped ≥ 0.0.
    /// e.g. 0.5 → entity deals 150% melee damage.
    pub damage_bonus_fraction: f32,
    /// Fraction of base movement speed lost while enlarged. Clamped [0.0, 1.0].
    /// e.g. 0.3 → entity moves at 70% speed.
    pub speed_penalty_fraction: f32,
    pub just_enlarged: bool,
    pub just_shrunken: bool,
    pub enabled: bool,
}

impl Enlarge {
    pub fn new(
        scale_multiplier: f32,
        damage_bonus_fraction: f32,
        speed_penalty_fraction: f32,
    ) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            scale_multiplier: scale_multiplier.max(1.0),
            damage_bonus_fraction: damage_bonus_fraction.max(0.0),
            speed_penalty_fraction: speed_penalty_fraction.clamp(0.0, 1.0),
            just_enlarged: false,
            just_shrunken: false,
            enabled: true,
        }
    }

    /// Grow the entity for `duration` seconds. High-watermark: only replaces
    /// the current timer when `duration > timer`. Sets `just_enlarged` on the
    /// inactive → active transition. No-op when disabled or `duration ≤ 0`.
    pub fn grow(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_enlarged = self.is_enlarged();
            self.duration = duration;
            self.timer = duration;
            if !was_enlarged {
                self.just_enlarged = true;
            }
        }
    }

    /// Return the entity to normal size early (e.g., ability cancelled or
    /// dispelled). Sets `just_shrunken`. No-op when not enlarged.
    pub fn shrink(&mut self) {
        if !self.is_enlarged() {
            return;
        }
        self.timer = 0.0;
        self.duration = 0.0;
        self.just_shrunken = true;
    }

    /// Advance the enlarge timer. Sets `just_shrunken` when the effect expires
    /// naturally. Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_enlarged = false;
        self.just_shrunken = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_shrunken = true;
            }
        }
    }

    pub fn is_enlarged(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective melee damage while enlarged.
    /// Returns `base * (1 + damage_bonus_fraction)` when enlarged and enabled,
    /// `base` otherwise.
    pub fn effective_damage(&self, base: f32) -> f32 {
        if self.is_enlarged() && self.enabled {
            base * (1.0 + self.damage_bonus_fraction)
        } else {
            base
        }
    }

    /// Effective movement speed while enlarged.
    /// Returns `base * (1 - speed_penalty_fraction)` when enlarged and enabled,
    /// floored at 0.0. Returns `base` otherwise.
    pub fn effective_speed(&self, base: f32) -> f32 {
        if self.is_enlarged() && self.enabled {
            (base * (1.0 - self.speed_penalty_fraction)).max(0.0)
        } else {
            base
        }
    }

    /// Fraction of the enlarge duration remaining [1.0 = just grew, 0.0 = normal size].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Enlarge {
    fn default() -> Self {
        Self::new(1.5, 0.5, 0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grow_starts_enlarge() {
        let mut e = Enlarge::new(1.5, 0.5, 0.3);
        e.grow(3.0);
        assert!(e.is_enlarged());
        assert!(e.just_enlarged);
    }

    #[test]
    fn grow_extends_on_longer_duration() {
        let mut e = Enlarge::new(1.5, 0.5, 0.3);
        e.grow(2.0);
        e.tick(0.016);
        e.grow(5.0);
        assert!((e.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn grow_no_extend_on_shorter_duration() {
        let mut e = Enlarge::new(1.5, 0.5, 0.3);
        e.grow(5.0);
        e.grow(2.0);
        assert!((e.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn just_enlarged_not_set_on_extend() {
        let mut e = Enlarge::new(1.5, 0.5, 0.3);
        e.grow(2.0);
        e.tick(0.016);
        e.grow(5.0);
        assert!(!e.just_enlarged);
    }

    #[test]
    fn shrink_ends_enlarge() {
        let mut e = Enlarge::new(1.5, 0.5, 0.3);
        e.grow(3.0);
        e.shrink();
        assert!(!e.is_enlarged());
        assert!(e.just_shrunken);
    }

    #[test]
    fn shrink_no_op_when_not_enlarged() {
        let mut e = Enlarge::new(1.5, 0.5, 0.3);
        e.shrink();
        assert!(!e.just_shrunken);
    }

    #[test]
    fn tick_expires_enlarge() {
        let mut e = Enlarge::new(1.5, 0.5, 0.3);
        e.grow(1.0);
        e.tick(1.1);
        assert!(!e.is_enlarged());
        assert!(e.just_shrunken);
    }

    #[test]
    fn tick_clears_just_enlarged() {
        let mut e = Enlarge::new(1.5, 0.5, 0.3);
        e.grow(3.0);
        e.tick(0.016);
        assert!(!e.just_enlarged);
    }

    #[test]
    fn tick_clears_just_shrunken() {
        let mut e = Enlarge::new(1.5, 0.5, 0.3);
        e.grow(0.5);
        e.tick(1.0);
        e.tick(0.016);
        assert!(!e.just_shrunken);
    }

    #[test]
    fn effective_damage_boosted_while_enlarged() {
        let mut e = Enlarge::new(1.5, 0.5, 0.3);
        e.grow(3.0);
        // 100 * (1 + 0.5) = 150
        assert!((e.effective_damage(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_damage_base_when_normal() {
        let e = Enlarge::new(1.5, 0.5, 0.3);
        assert!((e.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_speed_penalized_while_enlarged() {
        let mut e = Enlarge::new(1.5, 0.5, 0.3);
        e.grow(3.0);
        // 100 * (1 - 0.3) = 70
        assert!((e.effective_speed(100.0) - 70.0).abs() < 1e-3);
    }

    #[test]
    fn effective_speed_base_when_normal() {
        let e = Enlarge::new(1.5, 0.5, 0.3);
        assert!((e.effective_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_speed_floored_at_zero() {
        let mut e = Enlarge::new(1.5, 0.5, 1.0); // full penalty
        e.grow(3.0);
        assert!((e.effective_speed(100.0)).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut e = Enlarge::new(1.5, 0.5, 0.3);
        e.grow(4.0);
        e.tick(2.0);
        assert!((e.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_zero_when_normal() {
        let e = Enlarge::new(1.5, 0.5, 0.3);
        assert!((e.remaining_fraction()).abs() < 1e-5);
    }

    #[test]
    fn scale_multiplier_clamped_to_one() {
        let e = Enlarge::new(0.5, 0.5, 0.3);
        assert!((e.scale_multiplier - 1.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_grow_no_op() {
        let mut e = Enlarge::new(1.5, 0.5, 0.3);
        e.enabled = false;
        e.grow(3.0);
        assert!(!e.is_enlarged());
    }

    #[test]
    fn disabled_effective_damage_base() {
        let mut e = Enlarge::new(1.5, 0.5, 0.3);
        e.grow(3.0);
        e.enabled = false;
        assert!((e.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_effective_speed_base() {
        let mut e = Enlarge::new(1.5, 0.5, 0.3);
        e.grow(3.0);
        e.enabled = false;
        assert!((e.effective_speed(100.0) - 100.0).abs() < 1e-5);
    }
}
