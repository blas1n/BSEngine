use bevy_ecs::prelude::Component;

/// Health-inversely-proportional attack speed modifier: the more wounded the
/// entity, the faster it attacks. `fury_factor` runs from 0.0 (full health)
/// to 1.0 (empty health), and the attack speed bonus scales with it.
///
/// The health system calls `update(hp, max_hp)` every frame to recompute
/// `fury_factor`. `update` also fires `just_peaked` the first frame
/// `fury_factor` reaches 1.0 (entity is critical). It is a no-op when
/// disabled.
///
/// `tick()` clears `just_peaked` each frame.
///
/// `effective_attack_speed(base)` returns
/// `base * (1 + max_speed_bonus * fury_factor)` when enabled; returns `base`
/// when disabled or `fury_factor` is 0.
///
/// Distinct from `Rage` (discrete anger state entered voluntarily or
/// externally), `Ravage` (kill-triggered burst), and `Reckless` (static
/// offense/defense swap): Fury is a **continuous low-health escalation** —
/// the bonus accumulates smoothly as HP drains and peaks the instant the
/// entity would die, making it naturally most dangerous at the brink.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Fury {
    /// Current fury level [0.0, 1.0]. Updated by `update(hp, max_hp)`.
    pub fury_factor: f32,
    /// Maximum attack speed bonus fraction at full fury. Clamped ≥ 0.0.
    pub max_speed_bonus: f32,
    pub just_peaked: bool,
    pub enabled: bool,
}

impl Fury {
    pub fn new(max_speed_bonus: f32) -> Self {
        Self {
            fury_factor: 0.0,
            max_speed_bonus: max_speed_bonus.max(0.0),
            just_peaked: false,
            enabled: true,
        }
    }

    /// Recompute `fury_factor` as `1 - (hp / max_hp)`, clamped to [0.0, 1.0].
    /// Fires `just_peaked` on the first frame `fury_factor` reaches 1.0.
    /// `max_hp` is treated as 1.0 when ≤ 0 to avoid division by zero.
    /// No-op when disabled.
    pub fn update(&mut self, hp: f32, max_hp: f32) {
        if !self.enabled {
            return;
        }
        let effective_max = if max_hp > 0.0 { max_hp } else { 1.0 };
        let prev = self.fury_factor;
        self.fury_factor = (1.0 - (hp / effective_max)).clamp(0.0, 1.0);
        if prev < 1.0 && self.fury_factor >= 1.0 {
            self.just_peaked = true;
        }
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_peaked = false;
    }

    /// `true` when the entity has any fury and the component is enabled.
    pub fn is_furious(&self) -> bool {
        self.fury_factor > 0.0 && self.enabled
    }

    /// Effective attack speed with fury bonus applied.
    /// Returns `base * (1 + max_speed_bonus * fury_factor)` when enabled;
    /// returns `base` otherwise.
    pub fn effective_attack_speed(&self, base: f32) -> f32 {
        if self.enabled {
            base * (1.0 + self.max_speed_bonus * self.fury_factor)
        } else {
            base
        }
    }
}

impl Default for Fury {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_no_fury() {
        let f = Fury::new(1.0);
        assert!((f.fury_factor).abs() < 1e-5);
        assert!(!f.is_furious());
    }

    #[test]
    fn update_full_health_no_fury() {
        let mut f = Fury::new(1.0);
        f.update(100.0, 100.0);
        assert!((f.fury_factor).abs() < 1e-5);
        assert!(!f.is_furious());
    }

    #[test]
    fn update_half_health_half_fury() {
        let mut f = Fury::new(1.0);
        f.update(50.0, 100.0);
        assert!((f.fury_factor - 0.5).abs() < 1e-5);
        assert!(f.is_furious());
    }

    #[test]
    fn update_zero_health_full_fury() {
        let mut f = Fury::new(1.0);
        f.update(0.0, 100.0);
        assert!((f.fury_factor - 1.0).abs() < 1e-5);
        assert!(f.just_peaked);
    }

    #[test]
    fn update_negative_health_clamps_to_full_fury() {
        let mut f = Fury::new(1.0);
        f.update(-10.0, 100.0);
        assert!((f.fury_factor - 1.0).abs() < 1e-5);
    }

    #[test]
    fn just_peaked_not_set_again_when_already_at_one() {
        let mut f = Fury::new(1.0);
        f.update(0.0, 100.0); // peaks
        f.tick();
        f.update(0.0, 100.0); // still at 1.0 — no repeat
        assert!(!f.just_peaked);
    }

    #[test]
    fn just_peaked_set_again_after_recovery_and_redrop() {
        let mut f = Fury::new(1.0);
        f.update(0.0, 100.0); // peak
        f.tick();
        f.update(50.0, 100.0); // recover to 0.5
        f.update(0.0, 100.0); // peak again
        assert!(f.just_peaked);
    }

    #[test]
    fn tick_clears_just_peaked() {
        let mut f = Fury::new(1.0);
        f.update(0.0, 100.0);
        f.tick();
        assert!(!f.just_peaked);
    }

    #[test]
    fn update_no_op_when_disabled() {
        let mut f = Fury::new(1.0);
        f.enabled = false;
        f.update(0.0, 100.0);
        assert!((f.fury_factor).abs() < 1e-5);
    }

    #[test]
    fn is_furious_false_when_disabled() {
        let mut f = Fury::new(1.0);
        f.update(50.0, 100.0);
        f.enabled = false;
        assert!(!f.is_furious());
    }

    #[test]
    fn effective_attack_speed_scales_with_fury() {
        let mut f = Fury::new(1.0);
        f.update(0.0, 100.0); // fury_factor = 1.0
                              // 100 * (1 + 1.0 * 1.0) = 200
        assert!((f.effective_attack_speed(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_attack_speed_at_half_fury() {
        let mut f = Fury::new(1.0);
        f.update(50.0, 100.0); // fury_factor = 0.5
                               // 100 * (1 + 1.0 * 0.5) = 150
        assert!((f.effective_attack_speed(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_attack_speed_base_when_no_fury() {
        let f = Fury::new(1.0);
        assert!((f.effective_attack_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_attack_speed_base_when_disabled() {
        let mut f = Fury::new(1.0);
        f.update(0.0, 100.0);
        f.enabled = false;
        assert!((f.effective_attack_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_attack_speed_with_partial_bonus() {
        let mut f = Fury::new(0.5);
        f.update(0.0, 100.0); // fury_factor = 1.0
                              // 100 * (1 + 0.5 * 1.0) = 150
        assert!((f.effective_attack_speed(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn update_zero_max_hp_uses_fallback() {
        let mut f = Fury::new(1.0);
        f.update(0.0, 0.0); // avoids div-by-zero; hp=0, max=1 → fury=1
        assert!((f.fury_factor - 1.0).abs() < 1e-5);
    }

    #[test]
    fn update_tracks_recovery() {
        let mut f = Fury::new(1.0);
        f.update(10.0, 100.0); // 90% fury
        f.tick();
        f.update(80.0, 100.0); // 20% fury
        assert!((f.fury_factor - 0.2).abs() < 1e-4);
        assert!(!f.just_peaked);
    }

    #[test]
    fn max_speed_bonus_clamped_non_negative() {
        let f = Fury::new(-0.5);
        assert_eq!(f.max_speed_bonus, 0.0);
    }
}
