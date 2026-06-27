use bevy_ecs::prelude::Component;

/// Horizontal arc cleave: entity swings through multiple enemies in a wide
/// arc, dealing progressively reduced damage to each successive target.
///
/// The combat system detects enemies within `arc_width` degrees of the
/// entity's facing direction (up to `max_targets`) and calls
/// `damage_for_target(base, index)` for each, where `index` 0 is the
/// primary target.
///
/// `cleave()` fires `just_cleaved` to signal animation and VFX systems that
/// a cleave swing is occurring this frame. No-op when disabled.
///
/// `tick()` clears `just_cleaved` each frame.
///
/// Distinct from `Melee` (generic melee state without arc parameters),
/// `Slam` (vertical ground-based area attack), `Splinter` (fragment
/// scatter), and `Pulse` (omnidirectional area burst): Hew is a
/// **directional arc cleave** — damage originates from one direction and
/// fans out, hitting multiple targets in sequence with diminishing returns.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Hew {
    /// Full cleave arc width in degrees. Clamped [0.0, 360.0].
    pub arc_width: f32,
    /// Maximum enemies struck per swing. Clamped ≥ 1.
    pub max_targets: u32,
    /// Damage multiplier applied per successive target after the first.
    /// e.g. `0.75` means 2nd target takes 75%, 3rd takes ~56%. Clamped [0.0, 1.0].
    pub damage_falloff: f32,
    pub just_cleaved: bool,
    pub enabled: bool,
}

impl Hew {
    pub fn new(arc_width: f32, max_targets: u32, damage_falloff: f32) -> Self {
        Self {
            arc_width: arc_width.clamp(0.0, 360.0),
            max_targets: max_targets.max(1),
            damage_falloff: damage_falloff.clamp(0.0, 1.0),
            just_cleaved: false,
            enabled: true,
        }
    }

    /// Signal that a cleave swing is occurring this frame. Fires
    /// `just_cleaved`. No-op when disabled.
    pub fn cleave(&mut self) {
        if !self.enabled {
            return;
        }
        self.just_cleaved = true;
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_cleaved = false;
    }

    /// Damage dealt to the target at `index` (0 = primary). Returns
    /// `base * damage_falloff^index` floored at `0.0`, or `0.0` when
    /// `index >= max_targets` or the component is disabled.
    pub fn damage_for_target(&self, base: f32, index: u32) -> f32 {
        if !self.enabled || index >= self.max_targets {
            return 0.0;
        }
        if index == 0 {
            return base.max(0.0);
        }
        (base * self.damage_falloff.powi(index as i32)).max(0.0)
    }

    /// Whether the entity can currently cleave targets.
    pub fn can_cleave(&self) -> bool {
        self.enabled
    }
}

impl Default for Hew {
    fn default() -> Self {
        Self::new(90.0, 3, 0.75)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_has_correct_fields() {
        let h = Hew::new(90.0, 3, 0.75);
        assert!((h.arc_width - 90.0).abs() < 1e-5);
        assert_eq!(h.max_targets, 3);
        assert!((h.damage_falloff - 0.75).abs() < 1e-5);
        assert!(!h.just_cleaved);
    }

    #[test]
    fn cleave_fires_just_cleaved() {
        let mut h = Hew::new(90.0, 3, 0.75);
        h.cleave();
        assert!(h.just_cleaved);
    }

    #[test]
    fn cleave_no_op_when_disabled() {
        let mut h = Hew::new(90.0, 3, 0.75);
        h.enabled = false;
        h.cleave();
        assert!(!h.just_cleaved);
    }

    #[test]
    fn tick_clears_just_cleaved() {
        let mut h = Hew::new(90.0, 3, 0.75);
        h.cleave();
        h.tick();
        assert!(!h.just_cleaved);
    }

    #[test]
    fn damage_for_target_primary_is_full_damage() {
        let h = Hew::new(90.0, 3, 0.75);
        assert!((h.damage_for_target(100.0, 0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn damage_for_target_second_applies_falloff() {
        let h = Hew::new(90.0, 3, 0.75);
        // 100 * 0.75^1 = 75
        assert!((h.damage_for_target(100.0, 1) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn damage_for_target_third_applies_double_falloff() {
        let h = Hew::new(90.0, 3, 0.75);
        // 100 * 0.75^2 = 56.25
        assert!((h.damage_for_target(100.0, 2) - 56.25).abs() < 1e-3);
    }

    #[test]
    fn damage_for_target_beyond_max_returns_zero() {
        let h = Hew::new(90.0, 3, 0.75);
        assert!((h.damage_for_target(100.0, 3)).abs() < 1e-5);
        assert!((h.damage_for_target(100.0, 10)).abs() < 1e-5);
    }

    #[test]
    fn damage_for_target_zero_when_disabled() {
        let mut h = Hew::new(90.0, 3, 0.75);
        h.enabled = false;
        assert!((h.damage_for_target(100.0, 0)).abs() < 1e-5);
    }

    #[test]
    fn damage_for_target_floored_at_zero() {
        let h = Hew::new(90.0, 3, 0.75);
        assert!((h.damage_for_target(-100.0, 0)).abs() < 1e-5);
    }

    #[test]
    fn damage_falloff_zero_kills_splash_immediately() {
        let h = Hew::new(90.0, 3, 0.0);
        assert!((h.damage_for_target(100.0, 0) - 100.0).abs() < 1e-5);
        assert!((h.damage_for_target(100.0, 1)).abs() < 1e-5);
    }

    #[test]
    fn damage_falloff_one_deals_equal_to_all() {
        let h = Hew::new(90.0, 3, 1.0);
        assert!((h.damage_for_target(100.0, 0) - 100.0).abs() < 1e-5);
        assert!((h.damage_for_target(100.0, 1) - 100.0).abs() < 1e-5);
        assert!((h.damage_for_target(100.0, 2) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn arc_width_clamped_at_360() {
        let h = Hew::new(720.0, 3, 0.75);
        assert!((h.arc_width - 360.0).abs() < 1e-5);
    }

    #[test]
    fn arc_width_clamped_at_zero() {
        let h = Hew::new(-10.0, 3, 0.75);
        assert_eq!(h.arc_width, 0.0);
    }

    #[test]
    fn max_targets_clamped_to_one() {
        let h = Hew::new(90.0, 0, 0.75);
        assert_eq!(h.max_targets, 1);
    }

    #[test]
    fn damage_falloff_clamped_at_one() {
        let h = Hew::new(90.0, 3, 2.0);
        assert!((h.damage_falloff - 1.0).abs() < 1e-5);
    }

    #[test]
    fn damage_falloff_clamped_at_zero() {
        let h = Hew::new(90.0, 3, -0.5);
        assert_eq!(h.damage_falloff, 0.0);
    }

    #[test]
    fn can_cleave_true_when_enabled() {
        let h = Hew::new(90.0, 3, 0.75);
        assert!(h.can_cleave());
    }

    #[test]
    fn can_cleave_false_when_disabled() {
        let mut h = Hew::new(90.0, 3, 0.75);
        h.enabled = false;
        assert!(!h.can_cleave());
    }
}
