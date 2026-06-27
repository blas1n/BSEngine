use bevy_ecs::prelude::Component;

/// Accumulation of permanent wound marks that progressively cap health
/// regeneration.
///
/// Each scar reduces the fraction of maximum health this entity can regenerate
/// toward. Damage systems call `inflict()` after dealing a wound; healing
/// systems call `effective_regen(base)` to get the scarred regen rate. Scars
/// are removed by `cleanse(count)`.
///
/// `tick()` clears one-frame flags `just_scarred` and `just_cleansed`.
///
/// Distinct from `Bleed` (active damage-over-time), `Wound` (temporary
/// vulnerability), `Corrosion` (armor-degradation stack), and `Lacerate`
/// (attack component that inflicts cuts): Scar is a **permanent wound mark** —
/// it doesn't deal damage or reduce defence directly, but it permanently lowers
/// the ceiling of health regeneration until the scars are cleansed.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Scar {
    pub scars: u32,
    pub max_scars: u32,
    /// Fraction of base regen rate lost per scar. Clamped to [0.0, 1.0].
    /// e.g. 0.1 = each scar removes 10% of healing rate.
    pub regen_penalty_per_scar: f32,
    pub just_scarred: bool,
    pub just_cleansed: bool,
    pub enabled: bool,
}

impl Scar {
    pub fn new(regen_penalty_per_scar: f32, max_scars: u32) -> Self {
        Self {
            scars: 0,
            max_scars: max_scars.max(1),
            regen_penalty_per_scar: regen_penalty_per_scar.clamp(0.0, 1.0),
            just_scarred: false,
            just_cleansed: false,
            enabled: true,
        }
    }

    /// Add one scar (capped at `max_scars`). Sets `just_scarred` on the first
    /// scar (0 → 1 transition). No-op when disabled.
    pub fn inflict(&mut self) {
        if !self.enabled {
            return;
        }
        let was_zero = self.scars == 0;
        if self.scars < self.max_scars {
            self.scars += 1;
        }
        if was_zero && self.scars > 0 {
            self.just_scarred = true;
        }
    }

    /// Remove up to `count` scars. Sets `just_cleansed` when the last scar is
    /// removed (scars become 0).
    pub fn cleanse(&mut self, count: u32) {
        if count == 0 || self.scars == 0 {
            return;
        }
        let was_active = self.scars > 0;
        self.scars = self.scars.saturating_sub(count);
        if was_active && self.scars == 0 {
            self.just_cleansed = true;
        }
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_scarred = false;
        self.just_cleansed = false;
    }

    pub fn is_scarred(&self) -> bool {
        self.scars > 0
    }

    /// Total fraction of regen rate lost: `scars * regen_penalty_per_scar`,
    /// clamped to [0.0, 1.0].
    pub fn total_regen_penalty(&self) -> f32 {
        (self.scars as f32 * self.regen_penalty_per_scar).clamp(0.0, 1.0)
    }

    /// Effective regen after scarring. Returns `base * (1.0 - total_penalty)`
    /// when scarred and enabled, `base` otherwise. Never negative.
    pub fn effective_regen(&self, base: f32) -> f32 {
        if self.is_scarred() && self.enabled {
            (base * (1.0 - self.total_regen_penalty())).max(0.0)
        } else {
            base
        }
    }

    /// Fraction of max scars accumulated [0.0 = none, 1.0 = fully scarred].
    pub fn scar_fraction(&self) -> f32 {
        if self.max_scars == 0 {
            return 0.0;
        }
        (self.scars as f32 / self.max_scars as f32).clamp(0.0, 1.0)
    }
}

impl Default for Scar {
    fn default() -> Self {
        Self::new(0.1, 10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inflict_adds_scar() {
        let mut s = Scar::new(0.1, 10);
        s.inflict();
        assert_eq!(s.scars, 1);
        assert!(s.is_scarred());
        assert!(s.just_scarred);
    }

    #[test]
    fn inflict_caps_at_max() {
        let mut s = Scar::new(0.1, 3);
        for _ in 0..5 {
            s.inflict();
            s.tick();
        }
        assert_eq!(s.scars, 3);
    }

    #[test]
    fn just_scarred_only_on_first_scar() {
        let mut s = Scar::new(0.1, 10);
        s.inflict();
        s.tick();
        s.inflict(); // second scar
        assert!(!s.just_scarred);
    }

    #[test]
    fn cleanse_removes_scars() {
        let mut s = Scar::new(0.1, 10);
        s.inflict();
        s.inflict();
        s.inflict();
        s.cleanse(2);
        assert_eq!(s.scars, 1);
    }

    #[test]
    fn cleanse_sets_just_cleansed_on_last() {
        let mut s = Scar::new(0.1, 10);
        s.inflict();
        s.inflict();
        s.tick();
        s.cleanse(2);
        assert_eq!(s.scars, 0);
        assert!(s.just_cleansed);
    }

    #[test]
    fn cleanse_no_just_cleansed_when_scars_remain() {
        let mut s = Scar::new(0.1, 10);
        s.inflict();
        s.inflict();
        s.inflict();
        s.tick();
        s.cleanse(1); // 2 remain
        assert!(!s.just_cleansed);
    }

    #[test]
    fn cleanse_no_op_when_already_zero() {
        let mut s = Scar::new(0.1, 10);
        s.cleanse(1);
        assert!(!s.just_cleansed);
    }

    #[test]
    fn tick_clears_flags() {
        let mut s = Scar::new(0.1, 10);
        s.inflict();
        s.tick();
        assert!(!s.just_scarred);
        s.cleanse(1);
        s.tick();
        assert!(!s.just_cleansed);
    }

    #[test]
    fn total_regen_penalty_no_scars() {
        let s = Scar::new(0.1, 10);
        assert!((s.total_regen_penalty() - 0.0).abs() < 1e-5);
    }

    #[test]
    fn total_regen_penalty_with_scars() {
        let mut s = Scar::new(0.1, 10);
        s.inflict();
        s.inflict();
        s.inflict();
        // 3 * 0.1 = 0.3
        assert!((s.total_regen_penalty() - 0.3).abs() < 1e-5);
    }

    #[test]
    fn total_regen_penalty_clamped_at_one() {
        let mut s = Scar::new(0.5, 10);
        for _ in 0..10 {
            s.inflict();
            s.tick();
        }
        assert!((s.total_regen_penalty() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_regen_reduced() {
        let mut s = Scar::new(0.1, 10);
        s.inflict();
        s.inflict();
        // 20.0 * (1 - 0.2) = 16.0
        assert!((s.effective_regen(20.0) - 16.0).abs() < 1e-3);
    }

    #[test]
    fn effective_regen_full_when_unscarred() {
        let s = Scar::new(0.1, 10);
        assert!((s.effective_regen(20.0) - 20.0).abs() < 1e-5);
    }

    #[test]
    fn effective_regen_not_negative() {
        let mut s = Scar::new(1.0, 1);
        s.inflict();
        assert!(s.effective_regen(50.0) >= 0.0);
    }

    #[test]
    fn scar_fraction_at_half() {
        let mut s = Scar::new(0.1, 4);
        s.inflict();
        s.tick();
        s.inflict();
        assert!((s.scar_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn disabled_inflict_no_op() {
        let mut s = Scar::new(0.1, 10);
        s.enabled = false;
        s.inflict();
        assert_eq!(s.scars, 0);
    }

    #[test]
    fn disabled_effective_regen_full() {
        let mut s = Scar::new(0.1, 10);
        s.inflict();
        s.enabled = false;
        assert!((s.effective_regen(20.0) - 20.0).abs() < 1e-5);
    }
}
