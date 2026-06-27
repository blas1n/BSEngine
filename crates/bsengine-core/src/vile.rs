use bevy_ecs::prelude::Component;

/// Anti-heal debuff: while `is_vile()` and enabled, `effective_heal` reduces
/// incoming healing by `heal_reduction` fraction. Healing systems should call
/// `effective_heal(base)` instead of applying raw healing to a vile entity.
///
/// `apply(duration)` starts or extends the debuff (high-watermark); sets
/// `just_applied` on the inactive → active transition. `cleanse()` removes it
/// early. `tick(dt)` counts down and sets `just_cleared` on expiry.
///
/// Distinct from `Curse` (generic negative status), `Weaken` (damage
/// reduction), and `Drain` (HP leeching): Vile **reduces incoming healing
/// efficiency** — it silently poisons the recipient's recovery without altering
/// damage received or dealing damage directly.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Vile {
    pub duration: f32,
    pub timer: f32,
    /// Fraction of incoming healing blocked. Clamped [0.0, 1.0].
    pub heal_reduction: f32,
    pub just_applied: bool,
    pub just_cleared: bool,
    pub enabled: bool,
}

impl Vile {
    pub fn new(heal_reduction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            heal_reduction: heal_reduction.clamp(0.0, 1.0),
            just_applied: false,
            just_cleared: false,
            enabled: true,
        }
    }

    /// Apply or extend the debuff for `duration` seconds. High-watermark: only
    /// replaces the timer when `duration > timer`. Sets `just_applied` on the
    /// inactive → active transition. No-op when disabled or `duration ≤ 0`.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_vile = self.is_vile();
            self.duration = duration;
            self.timer = duration;
            if !was_vile {
                self.just_applied = true;
            }
        }
    }

    /// Remove the debuff early. Sets `just_cleared`. No-op when not active.
    pub fn cleanse(&mut self) {
        if !self.is_vile() {
            return;
        }
        self.timer = 0.0;
        self.duration = 0.0;
        self.just_cleared = true;
    }

    /// Advance the debuff timer. Sets `just_cleared` when the debuff expires
    /// naturally. Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_applied = false;
        self.just_cleared = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_cleared = true;
            }
        }
    }

    pub fn is_vile(&self) -> bool {
        self.timer > 0.0
    }

    /// Incoming healing after anti-heal reduction.
    /// Returns `base * (1 - heal_reduction)` floored at `0.0` while vile and
    /// enabled. Returns `base` unchanged otherwise.
    pub fn effective_heal(&self, base: f32) -> f32 {
        if self.is_vile() && self.enabled {
            (base * (1.0 - self.heal_reduction)).max(0.0)
        } else {
            base
        }
    }

    /// Fraction of the debuff duration remaining [1.0 = just applied, 0.0 = cleared].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Vile {
    fn default() -> Self {
        Self::new(0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_starts_debuff() {
        let mut v = Vile::new(0.5);
        v.apply(5.0);
        assert!(v.is_vile());
        assert!(v.just_applied);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut v = Vile::new(0.5);
        v.apply(3.0);
        v.tick(0.016);
        v.apply(8.0);
        assert!((v.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut v = Vile::new(0.5);
        v.apply(8.0);
        v.apply(3.0);
        assert!((v.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn just_applied_not_set_on_extend() {
        let mut v = Vile::new(0.5);
        v.apply(3.0);
        v.tick(0.016);
        v.apply(8.0);
        assert!(!v.just_applied);
    }

    #[test]
    fn cleanse_removes_debuff() {
        let mut v = Vile::new(0.5);
        v.apply(5.0);
        v.cleanse();
        assert!(!v.is_vile());
        assert!(v.just_cleared);
    }

    #[test]
    fn cleanse_no_op_when_not_vile() {
        let mut v = Vile::new(0.5);
        v.cleanse();
        assert!(!v.just_cleared);
    }

    #[test]
    fn tick_expires_debuff() {
        let mut v = Vile::new(0.5);
        v.apply(1.0);
        v.tick(1.1);
        assert!(!v.is_vile());
        assert!(v.just_cleared);
    }

    #[test]
    fn tick_clears_just_applied() {
        let mut v = Vile::new(0.5);
        v.apply(5.0);
        v.tick(0.016);
        assert!(!v.just_applied);
    }

    #[test]
    fn tick_clears_just_cleared() {
        let mut v = Vile::new(0.5);
        v.apply(0.5);
        v.tick(1.0);
        v.tick(0.016);
        assert!(!v.just_cleared);
    }

    #[test]
    fn effective_heal_reduced_when_vile() {
        let mut v = Vile::new(0.5);
        v.apply(5.0);
        // 100 * (1 - 0.5) = 50
        assert!((v.effective_heal(100.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn effective_heal_base_when_not_vile() {
        let v = Vile::new(0.5);
        assert!((v.effective_heal(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_heal_floored_at_zero() {
        let mut v = Vile::new(1.0);
        v.apply(5.0);
        assert!((v.effective_heal(100.0)).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut v = Vile::new(0.5);
        v.apply(6.0);
        v.tick(3.0);
        assert!((v.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_zero_when_not_vile() {
        let v = Vile::new(0.5);
        assert!((v.remaining_fraction()).abs() < 1e-5);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut v = Vile::new(0.5);
        v.enabled = false;
        v.apply(5.0);
        assert!(!v.is_vile());
    }

    #[test]
    fn disabled_effective_heal_base() {
        let mut v = Vile::new(0.5);
        v.apply(5.0);
        v.enabled = false;
        assert!((v.effective_heal(100.0) - 100.0).abs() < 1e-5);
    }
}
