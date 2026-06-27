use bevy_ecs::prelude::Component;

/// Stability/balance meter that absorbs the force of heavy attacks before
/// breaking.
///
/// Each heavy hit calls `damage(amount)` to reduce `current` toward zero.
/// When `current` reaches zero the poise breaks (`just_broken`), and the combat
/// system should apply a `Stagger` to the entity. Poise regenerates over time
/// via `tick(dt)` at `regen_rate` per second; `just_restored` fires when the
/// meter fills back to `max` after a previous break.
///
/// `restore(amount)` can be called externally (e.g., from a buff or consumable)
/// to refill poise immediately. Call `break_now()` to force an instant poise
/// break (e.g., from a grab or finisher).
///
/// Distinct from `Stamina` (action economy — governs dodging and blocking),
/// `Health` (HP pool), and `Stagger` (the applied debuff after a poise break):
/// Poise is the balance meter that, when depleted by heavy hits, results in
/// a stagger.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Poise {
    pub current: f32,
    pub max: f32,
    /// Poise regenerated per second. Clamped ≥ 0.0.
    pub regen_rate: f32,
    pub just_broken: bool,
    pub just_restored: bool,
    pub enabled: bool,
}

impl Poise {
    pub fn new(max: f32, regen_rate: f32) -> Self {
        let max = max.max(0.0);
        Self {
            current: max,
            max,
            regen_rate: regen_rate.max(0.0),
            just_broken: false,
            just_restored: false,
            enabled: true,
        }
    }

    /// Reduce poise by `amount`. Sets `just_broken` if `current` crosses zero.
    /// No-op when disabled or already broken.
    pub fn damage(&mut self, amount: f32) {
        if !self.enabled || self.is_broken() {
            return;
        }
        let was_ok = !self.is_broken();
        self.current = (self.current - amount.max(0.0)).max(0.0);
        if was_ok && self.is_broken() {
            self.just_broken = true;
        }
    }

    /// Force an immediate poise break regardless of current value. No-op when
    /// already broken or disabled.
    pub fn break_now(&mut self) {
        if !self.enabled || self.is_broken() {
            return;
        }
        self.current = 0.0;
        self.just_broken = true;
    }

    /// Restore poise by `amount`, clamped at `max`. Sets `just_restored` when
    /// the meter reaches `max` after a previous break (i.e., was 0 before).
    pub fn restore(&mut self, amount: f32) {
        let was_broken = self.is_broken();
        self.current = (self.current + amount.max(0.0)).min(self.max);
        if was_broken && !self.is_broken() {
            self.just_restored = true;
        }
    }

    /// Advance regen; sets `just_restored` when poise fully recovers after a
    /// break. Clears one-frame flags.
    pub fn tick(&mut self, dt: f32) {
        self.just_broken = false;
        self.just_restored = false;

        if self.enabled && self.regen_rate > 0.0 && self.current < self.max {
            let was_broken = self.is_broken();
            self.current = (self.current + self.regen_rate * dt).min(self.max);
            if was_broken && !self.is_broken() {
                self.just_restored = true;
            }
        }
    }

    pub fn is_broken(&self) -> bool {
        self.current <= 0.0
    }

    pub fn is_full(&self) -> bool {
        self.current >= self.max
    }

    /// Fraction of max poise remaining [0.0 = broken, 1.0 = full].
    pub fn fraction(&self) -> f32 {
        if self.max <= 0.0 {
            return 1.0;
        }
        (self.current / self.max).clamp(0.0, 1.0)
    }
}

impl Default for Poise {
    fn default() -> Self {
        Self::new(100.0, 20.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_reduces_current() {
        let mut p = Poise::new(100.0, 20.0);
        p.damage(30.0);
        assert!((p.current - 70.0).abs() < 1e-4);
        assert!(!p.just_broken);
    }

    #[test]
    fn damage_breaks_poise() {
        let mut p = Poise::new(100.0, 20.0);
        p.damage(100.0);
        assert!(p.is_broken());
        assert!(p.just_broken);
    }

    #[test]
    fn damage_does_not_go_below_zero() {
        let mut p = Poise::new(100.0, 20.0);
        p.damage(200.0);
        assert!((p.current - 0.0).abs() < 1e-5);
    }

    #[test]
    fn damage_no_op_when_already_broken() {
        let mut p = Poise::new(100.0, 20.0);
        p.damage(100.0);
        p.tick(0.016); // clear just_broken
        p.damage(50.0); // should be no-op
        assert!(!p.just_broken);
    }

    #[test]
    fn break_now_sets_broken() {
        let mut p = Poise::new(100.0, 20.0);
        p.break_now();
        assert!(p.is_broken());
        assert!(p.just_broken);
    }

    #[test]
    fn break_now_no_op_when_already_broken() {
        let mut p = Poise::new(100.0, 20.0);
        p.break_now();
        p.tick(0.016);
        p.break_now(); // already broken
        assert!(!p.just_broken);
    }

    #[test]
    fn restore_refills_poise() {
        let mut p = Poise::new(100.0, 20.0);
        p.damage(50.0);
        p.restore(30.0);
        assert!((p.current - 80.0).abs() < 1e-4);
    }

    #[test]
    fn restore_sets_just_restored_on_break_end() {
        let mut p = Poise::new(100.0, 20.0);
        p.damage(100.0); // break
        p.tick(0.016);
        p.restore(50.0); // partial — still broken? no, 50 > 0
        assert!(p.just_restored);
    }

    #[test]
    fn restore_clamps_at_max() {
        let mut p = Poise::new(100.0, 20.0);
        p.restore(200.0);
        assert!((p.current - 100.0).abs() < 1e-5);
    }

    #[test]
    fn tick_regenerates_poise() {
        let mut p = Poise::new(100.0, 20.0);
        p.damage(40.0);
        p.tick(1.0); // regen 20/s
        assert!((p.current - 80.0).abs() < 1e-3);
    }

    #[test]
    fn tick_sets_just_restored_when_fully_regened_after_break() {
        let mut p = Poise::new(10.0, 20.0);
        p.damage(10.0); // break
        p.tick(0.016); // clear just_broken
        p.tick(1.0); // regen 20/s → full
        assert!(p.just_restored);
        assert!(p.is_full());
    }

    #[test]
    fn tick_clears_just_broken() {
        let mut p = Poise::new(100.0, 20.0);
        p.damage(100.0);
        p.tick(0.016);
        assert!(!p.just_broken);
    }

    #[test]
    fn fraction_at_half() {
        let mut p = Poise::new(100.0, 20.0);
        p.damage(50.0);
        assert!((p.fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn fraction_zero_when_broken() {
        let mut p = Poise::new(100.0, 20.0);
        p.damage(100.0);
        assert!((p.fraction() - 0.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_damage_no_op() {
        let mut p = Poise::new(100.0, 20.0);
        p.enabled = false;
        p.damage(100.0);
        assert!(!p.is_broken());
    }

    #[test]
    fn disabled_regen_no_op() {
        let mut p = Poise::new(100.0, 20.0);
        p.damage(50.0);
        p.enabled = false;
        p.tick(10.0);
        assert!((p.current - 50.0).abs() < 1e-4);
    }
}
