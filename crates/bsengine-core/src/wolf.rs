use bevy_ecs::prelude::Component;

/// Hunt-intensity accumulator. Models a predator's escalating commitment to
/// a target: attack power builds as focus narrows on a single prey, but
/// scatters quickly when pursuit is broken.
///
/// `pursue()` begins active hunting; no-op if already hunting or disabled.
///
/// `break_off()` ends the pursuit; no-op if not currently hunting.
///
/// `tick(dt)` clears both one-frame flags first, then:
/// - If `hunting`: increases `hunt_level` by `hunt_rate * dt` (capped at
///   `max_hunt`); fires `just_locked` the first time it reaches the cap.
/// - If `!hunting` and `hunt_level > 0`: decreases by `scatter_rate * dt`
///   (floored at 0); fires `just_broken` the first time it reaches 0.
/// - No-op when disabled (flags are still cleared).
///
/// `is_locked()` returns `hunt_level >= max_hunt && enabled`.
///
/// `hunt_fraction()` returns `(hunt_level / max_hunt).clamp(0.0, 1.0)`.
///
/// `effective_attack(base)` returns
/// `base * (1.0 + attack_bonus * hunt_fraction())` when enabled;
/// returns `base` unchanged otherwise.
///
/// Distinct from `Rage` (combat-triggered escalation), `Fervor` (morale-
/// driven zeal), and `Rampage` (AoE momentum): Wolf models **focused
/// predatory commitment to a single target** — the longer the hunt, the
/// stronger the strike; lose the scent and the advantage dissolves fast.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wolf {
    /// Current hunt focus [0.0, max_hunt].
    pub hunt_level: f32,
    /// Maximum focus. Clamped >= 1.0.
    pub max_hunt: f32,
    /// Focus gain per second while hunting. Clamped >= 0.0.
    pub hunt_rate: f32,
    /// Focus loss per second when not hunting. Clamped >= 0.0.
    pub scatter_rate: f32,
    /// Attack bonus multiplier at full focus. Clamped >= 0.0.
    pub attack_bonus: f32,
    pub hunting: bool,
    pub just_locked: bool,
    pub just_broken: bool,
    pub enabled: bool,
}

impl Wolf {
    pub fn new(max_hunt: f32, hunt_rate: f32, scatter_rate: f32, attack_bonus: f32) -> Self {
        Self {
            hunt_level: 0.0,
            max_hunt: max_hunt.max(1.0),
            hunt_rate: hunt_rate.max(0.0),
            scatter_rate: scatter_rate.max(0.0),
            attack_bonus: attack_bonus.max(0.0),
            hunting: false,
            just_locked: false,
            just_broken: false,
            enabled: true,
        }
    }

    /// Begin pursuit. No-op if already hunting or disabled.
    pub fn pursue(&mut self) {
        if !self.enabled || self.hunting {
            return;
        }
        self.hunting = true;
    }

    /// End pursuit. No-op if not currently hunting.
    pub fn break_off(&mut self) {
        if !self.hunting {
            return;
        }
        self.hunting = false;
    }

    /// Advance one frame: clear flags, then build or scatter hunt focus.
    /// No-op (beyond flag clear) when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_locked = false;
        self.just_broken = false;

        if !self.enabled {
            return;
        }

        if self.hunting {
            let was_below = self.hunt_level < self.max_hunt;
            self.hunt_level = (self.hunt_level + self.hunt_rate * dt).min(self.max_hunt);
            if was_below && self.hunt_level >= self.max_hunt {
                self.just_locked = true;
            }
        } else if self.hunt_level > 0.0 {
            let was_above = self.hunt_level > 0.0;
            self.hunt_level = (self.hunt_level - self.scatter_rate * dt).max(0.0);
            if was_above && self.hunt_level <= 0.0 {
                self.just_broken = true;
            }
        }
    }

    /// `true` when hunt focus is at maximum and component is enabled.
    pub fn is_locked(&self) -> bool {
        self.hunt_level >= self.max_hunt && self.enabled
    }

    /// Hunt focus as a fraction of maximum [0.0, 1.0].
    pub fn hunt_fraction(&self) -> f32 {
        (self.hunt_level / self.max_hunt).clamp(0.0, 1.0)
    }

    /// Scale attack `base` by hunt focus. Returns
    /// `base * (1.0 + attack_bonus * fraction)` when enabled; `base`
    /// otherwise.
    pub fn effective_attack(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.attack_bonus * self.hunt_fraction())
    }
}

impl Default for Wolf {
    fn default() -> Self {
        Self::new(10.0, 3.0, 5.0, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wolf {
        Wolf::new(10.0, 3.0, 5.0, 0.5)
    }

    #[test]
    fn new_starts_idle() {
        let w = w();
        assert_eq!(w.hunt_level, 0.0);
        assert!(!w.hunting);
        assert!(!w.just_locked);
        assert!(!w.just_broken);
        assert!(!w.is_locked());
    }

    #[test]
    fn pursue_sets_hunting() {
        let mut w = w();
        w.pursue();
        assert!(w.hunting);
    }

    #[test]
    fn pursue_no_op_when_already_hunting() {
        let mut w = w();
        w.pursue();
        w.tick(1.0); // 3.0
        w.pursue(); // no-op
        assert!((w.hunt_level - 3.0).abs() < 1e-4);
    }

    #[test]
    fn pursue_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.pursue();
        assert!(!w.hunting);
    }

    #[test]
    fn break_off_clears_hunting() {
        let mut w = w();
        w.pursue();
        w.break_off();
        assert!(!w.hunting);
    }

    #[test]
    fn break_off_no_op_when_not_hunting() {
        let mut w = w();
        w.break_off();
        assert!(!w.hunting);
    }

    #[test]
    fn tick_builds_focus_while_hunting() {
        let mut w = w(); // hunt_rate=3.0
        w.pursue();
        w.tick(1.0);
        assert!((w.hunt_level - 3.0).abs() < 1e-4);
    }

    #[test]
    fn tick_caps_focus_at_max() {
        let mut w = w();
        w.pursue();
        w.tick(100.0); // capped at 10
        assert!((w.hunt_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_no_build_without_pursue() {
        let mut w = w();
        w.tick(5.0);
        assert_eq!(w.hunt_level, 0.0);
    }

    #[test]
    fn tick_scatters_focus_after_break_off() {
        let mut w = w(); // scatter_rate=5.0
        w.pursue();
        w.tick(2.0); // 6.0
        w.break_off();
        w.tick(1.0); // 6.0 - 5.0 = 1.0
        assert!((w.hunt_level - 1.0).abs() < 1e-4);
    }

    #[test]
    fn tick_floors_scatter_at_zero() {
        let mut w = w();
        w.pursue();
        w.tick(1.0); // 3.0
        w.break_off();
        w.tick(100.0); // 0
        assert_eq!(w.hunt_level, 0.0);
    }

    #[test]
    fn tick_no_op_when_disabled_no_build() {
        let mut w = w();
        w.pursue();
        w.enabled = false;
        w.tick(5.0);
        assert_eq!(w.hunt_level, 0.0);
    }

    #[test]
    fn tick_no_op_when_disabled_no_scatter() {
        let mut w = w();
        w.pursue();
        w.tick(2.0); // 6.0
        w.enabled = false;
        w.tick(5.0); // no scatter
        assert!((w.hunt_level - 6.0).abs() < 1e-4);
    }

    #[test]
    fn tick_clears_flags_even_when_disabled() {
        let mut w = w();
        w.just_locked = true;
        w.just_broken = true;
        w.enabled = false;
        w.tick(0.016);
        assert!(!w.just_locked);
        assert!(!w.just_broken);
    }

    #[test]
    fn just_locked_fires_at_max() {
        let mut w = w(); // hunt_rate=3, max=10 → 3.34s to lock
        w.pursue();
        w.tick(4.0); // 12 → cap → just_locked
        assert!(w.just_locked);
    }

    #[test]
    fn just_locked_clears_next_tick() {
        let mut w = w();
        w.pursue();
        w.tick(4.0); // locked
        w.tick(0.016);
        assert!(!w.just_locked);
    }

    #[test]
    fn just_locked_fires_only_once_at_max() {
        let mut w = w();
        w.pursue();
        w.tick(4.0); // locked
        w.tick(0.016); // cleared
        w.tick(1.0); // still at max, no re-fire
        assert!(!w.just_locked);
    }

    #[test]
    fn just_broken_fires_at_zero() {
        let mut w = w(); // scatter_rate=5
        w.pursue();
        w.tick(1.0); // 3.0
        w.break_off();
        w.tick(1.0); // 3.0 - 5.0 → 0 → just_broken
        assert!(w.just_broken);
    }

    #[test]
    fn just_broken_clears_next_tick() {
        let mut w = w();
        w.pursue();
        w.tick(1.0); // 3.0
        w.break_off();
        w.tick(1.0); // broken
        w.tick(0.016); // cleared
        assert!(!w.just_broken);
    }

    #[test]
    fn just_broken_fires_only_once_at_zero() {
        let mut w = w();
        w.pursue();
        w.tick(1.0); // 3.0
        w.break_off();
        w.tick(1.0); // 0 — fires
        w.tick(0.016); // cleared
        w.tick(1.0); // still 0, no re-fire
        assert!(!w.just_broken);
    }

    #[test]
    fn is_locked_true_at_max() {
        let mut w = w();
        w.pursue();
        w.tick(100.0);
        assert!(w.is_locked());
    }

    #[test]
    fn is_locked_false_below_max() {
        let mut w = w();
        w.pursue();
        w.tick(1.0); // 3.0 < 10.0
        assert!(!w.is_locked());
    }

    #[test]
    fn is_locked_false_when_disabled() {
        let mut w = w();
        w.pursue();
        w.tick(100.0);
        w.enabled = false;
        assert!(!w.is_locked());
    }

    #[test]
    fn hunt_fraction_zero_when_idle() {
        let w = w();
        assert_eq!(w.hunt_fraction(), 0.0);
    }

    #[test]
    fn hunt_fraction_half_at_midpoint() {
        let mut w = w();
        w.hunt_level = 5.0;
        assert!((w.hunt_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn hunt_fraction_one_at_max() {
        let mut w = w();
        w.pursue();
        w.tick(100.0);
        assert!((w.hunt_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn effective_attack_base_when_idle() {
        let w = w(); // no focus → no bonus
        assert!((w.effective_attack(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_attack_boosted_at_half_focus() {
        let mut w = Wolf::new(10.0, 3.0, 5.0, 0.5);
        w.hunt_level = 5.0; // fraction=0.5
                            // 100 * (1 + 0.5*0.5) = 125
        assert!((w.effective_attack(100.0) - 125.0).abs() < 1e-3);
    }

    #[test]
    fn effective_attack_fully_boosted_at_max() {
        let mut w = Wolf::new(10.0, 3.0, 5.0, 0.5);
        w.pursue();
        w.tick(100.0); // max
                       // 100 * (1 + 0.5*1.0) = 150
        assert!((w.effective_attack(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_attack_passthrough_when_disabled() {
        let mut w = w();
        w.pursue();
        w.tick(100.0);
        w.enabled = false;
        assert!((w.effective_attack(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn max_hunt_clamped_to_one() {
        let w = Wolf::new(0.0, 3.0, 5.0, 0.5);
        assert!((w.max_hunt - 1.0).abs() < 1e-5);
    }

    #[test]
    fn hunt_rate_clamped_to_zero() {
        let w = Wolf::new(10.0, -3.0, 5.0, 0.5);
        assert_eq!(w.hunt_rate, 0.0);
    }

    #[test]
    fn scatter_rate_clamped_to_zero() {
        let w = Wolf::new(10.0, 3.0, -5.0, 0.5);
        assert_eq!(w.scatter_rate, 0.0);
    }

    #[test]
    fn attack_bonus_clamped_to_zero() {
        let w = Wolf::new(10.0, 3.0, 5.0, -1.0);
        assert_eq!(w.attack_bonus, 0.0);
    }

    #[test]
    fn pursue_break_pursue_cycle() {
        let mut w = w(); // hunt_rate=3, scatter_rate=5
        w.pursue();
        w.tick(2.0); // 6.0
        w.break_off();
        w.tick(0.5); // 6.0 - 2.5 = 3.5
        w.pursue();
        w.tick(1.0); // 3.5 + 3.0 = 6.5
        assert!((w.hunt_level - 6.5).abs() < 1e-4);
    }
}
