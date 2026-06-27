use bevy_ecs::prelude::Component;

/// Enhanced-grip climbing modifier: while `active`, the entity's climb speed
/// gains a `grip_bonus` fraction and its downward slide speed is reduced by
/// `slip_resistance`. The movement system sets active state via
/// `set_gripping(true/false)`; one-frame event flags fire on each transition.
///
/// `effective_climb_speed(base)` returns `base * (1 + grip_bonus)` while
/// gripping and enabled; `effective_slide_speed(base)` returns
/// `base * (1 - slip_resistance)` floored at `0.0`. Both return `base`
/// when inactive or disabled.
///
/// Distinct from `Climb` (enables climbing at all), `Grapple` (hook-and-swing
/// locomotion), and `Hover` (aerial float): Talon is an **additive grip
/// enhancement** — it improves the quality of climbing already enabled by
/// other components, not the ability to climb itself.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Talon {
    /// Whether the entity is currently gripping a surface.
    pub active: bool,
    /// Fraction added to climb speed while gripping. Clamped ≥ 0.0.
    pub grip_bonus: f32,
    /// Fraction of downward slide speed removed while gripping. Clamped [0.0, 1.0].
    /// 0.0 = full slide; 1.0 = no slide at all.
    pub slip_resistance: f32,
    pub just_gripped: bool,
    pub just_released: bool,
    pub enabled: bool,
}

impl Talon {
    pub fn new(grip_bonus: f32, slip_resistance: f32) -> Self {
        Self {
            active: false,
            grip_bonus: grip_bonus.max(0.0),
            slip_resistance: slip_resistance.clamp(0.0, 1.0),
            just_gripped: false,
            just_released: false,
            enabled: true,
        }
    }

    /// Update whether the entity is gripping. Fires `just_gripped` on the
    /// inactive → active transition and `just_released` on active → inactive.
    /// No-op when called with the same state. `engage` (false → true) is also
    /// ignored when disabled.
    pub fn set_gripping(&mut self, gripping: bool) {
        if gripping == self.active {
            return;
        }
        if gripping && !self.enabled {
            return;
        }
        self.active = gripping;
        if gripping {
            self.just_gripped = true;
        } else {
            self.just_released = true;
        }
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_gripped = false;
        self.just_released = false;
    }

    /// Effective climb speed: `base * (1 + grip_bonus)` while gripping and
    /// enabled. Returns `base` otherwise.
    pub fn effective_climb_speed(&self, base: f32) -> f32 {
        if self.active && self.enabled {
            base * (1.0 + self.grip_bonus)
        } else {
            base
        }
    }

    /// Effective downward slide speed: `base * (1 - slip_resistance)` while
    /// gripping and enabled, floored at `0.0`. Returns `base` otherwise.
    pub fn effective_slide_speed(&self, base: f32) -> f32 {
        if self.active && self.enabled {
            (base * (1.0 - self.slip_resistance)).max(0.0)
        } else {
            base
        }
    }

    pub fn is_gripping(&self) -> bool {
        self.active && self.enabled
    }
}

impl Default for Talon {
    fn default() -> Self {
        Self::new(0.5, 0.8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_gripping_true_activates_and_flags() {
        let mut t = Talon::new(0.5, 0.8);
        t.set_gripping(true);
        assert!(t.active);
        assert!(t.just_gripped);
        assert!(!t.just_released);
    }

    #[test]
    fn set_gripping_false_deactivates_and_flags() {
        let mut t = Talon::new(0.5, 0.8);
        t.set_gripping(true);
        t.tick();
        t.set_gripping(false);
        assert!(!t.active);
        assert!(t.just_released);
        assert!(!t.just_gripped);
    }

    #[test]
    fn set_gripping_no_op_when_same_state() {
        let mut t = Talon::new(0.5, 0.8);
        t.set_gripping(false); // already inactive
        assert!(!t.just_released);
        t.set_gripping(true);
        t.tick();
        t.set_gripping(true); // already active
        assert!(!t.just_gripped);
    }

    #[test]
    fn set_gripping_true_no_op_when_disabled() {
        let mut t = Talon::new(0.5, 0.8);
        t.enabled = false;
        t.set_gripping(true);
        assert!(!t.active);
        assert!(!t.just_gripped);
    }

    #[test]
    fn set_gripping_false_fires_released_even_when_disabled() {
        let mut t = Talon::new(0.5, 0.8);
        t.set_gripping(true);
        t.enabled = false;
        t.set_gripping(false);
        assert!(!t.active);
        assert!(t.just_released);
    }

    #[test]
    fn tick_clears_just_gripped() {
        let mut t = Talon::new(0.5, 0.8);
        t.set_gripping(true);
        t.tick();
        assert!(!t.just_gripped);
    }

    #[test]
    fn tick_clears_just_released() {
        let mut t = Talon::new(0.5, 0.8);
        t.set_gripping(true);
        t.set_gripping(false);
        t.tick();
        assert!(!t.just_released);
    }

    #[test]
    fn effective_climb_speed_boosted_while_gripping() {
        let mut t = Talon::new(0.5, 0.8);
        t.set_gripping(true);
        // 100 * (1 + 0.5) = 150
        assert!((t.effective_climb_speed(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_climb_speed_base_when_not_gripping() {
        let t = Talon::new(0.5, 0.8);
        assert!((t.effective_climb_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_climb_speed_base_when_disabled() {
        let mut t = Talon::new(0.5, 0.8);
        t.set_gripping(true);
        t.enabled = false;
        assert!((t.effective_climb_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_slide_speed_reduced_while_gripping() {
        let mut t = Talon::new(0.5, 0.8);
        t.set_gripping(true);
        // 100 * (1 - 0.8) = 20
        assert!((t.effective_slide_speed(100.0) - 20.0).abs() < 1e-3);
    }

    #[test]
    fn effective_slide_speed_zero_at_full_resistance() {
        let mut t = Talon::new(0.5, 1.0);
        t.set_gripping(true);
        assert!((t.effective_slide_speed(100.0)).abs() < 1e-5);
    }

    #[test]
    fn effective_slide_speed_base_when_not_gripping() {
        let t = Talon::new(0.5, 0.8);
        assert!((t.effective_slide_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_slide_speed_base_when_disabled() {
        let mut t = Talon::new(0.5, 0.8);
        t.set_gripping(true);
        t.enabled = false;
        assert!((t.effective_slide_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn is_gripping_requires_active_and_enabled() {
        let mut t = Talon::new(0.5, 0.8);
        assert!(!t.is_gripping());
        t.set_gripping(true);
        assert!(t.is_gripping());
        t.enabled = false;
        assert!(!t.is_gripping());
    }

    #[test]
    fn zero_grip_bonus_unchanged_speed() {
        let mut t = Talon::new(0.0, 0.5);
        t.set_gripping(true);
        assert!((t.effective_climb_speed(100.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn grip_bonus_clamped_non_negative() {
        let t = Talon::new(-1.0, 0.5);
        assert!(t.grip_bonus >= 0.0);
    }
}
