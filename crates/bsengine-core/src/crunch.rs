use bevy_ecs::prelude::Component;

/// Turtle-mode defensive stance: while braced, all incoming damage is reduced
/// by `damage_reduction` at the cost of full mobility. The physics system is
/// responsible for zeroing velocity while `is_braced()` returns `true`.
///
/// `crunch()` enters the stance and fires `just_braced`. `stand()` exits it
/// and fires `just_broke`. `tick()` clears one-frame flags at the start of
/// each call.
///
/// `crunch()` is a no-op when already braced or disabled. `stand()` is a
/// no-op when not braced.
///
/// Distinct from `Crouch` (stealth/evasion movement stance), `Shield`
/// (personal energy barrier), `Invincible` (complete immunity), and
/// `Protect` (intercepts ally damage): Crunch is a **turtle-mode defensive
/// stance** — maximum personal damage reduction at the full cost of mobility.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Crunch {
    pub active: bool,
    /// Fraction of incoming damage absorbed. Clamped [0.0, 1.0].
    /// e.g. 0.75 means the entity takes only 25% of incoming damage.
    pub damage_reduction: f32,
    pub just_braced: bool,
    pub just_broke: bool,
    pub enabled: bool,
}

impl Crunch {
    pub fn new(damage_reduction: f32) -> Self {
        Self {
            active: false,
            damage_reduction: damage_reduction.clamp(0.0, 1.0),
            just_braced: false,
            just_broke: false,
            enabled: true,
        }
    }

    /// Enter the defensive stance. Fires `just_braced`. No-op when already
    /// braced or disabled.
    pub fn crunch(&mut self) {
        if self.active || !self.enabled {
            return;
        }
        self.active = true;
        self.just_braced = true;
    }

    /// Exit the defensive stance. Fires `just_broke`. No-op when not braced.
    pub fn stand(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        self.just_broke = true;
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_braced = false;
        self.just_broke = false;
    }

    /// `true` while in the defensive stance and enabled.
    pub fn is_braced(&self) -> bool {
        self.active && self.enabled
    }

    /// Effective incoming damage after stance reduction.
    /// Returns `incoming * (1 - damage_reduction)` while braced and enabled,
    /// floored at 0.0. Returns `incoming` otherwise.
    pub fn damage_taken(&self, incoming: f32) -> f32 {
        if self.is_braced() {
            (incoming * (1.0 - self.damage_reduction)).max(0.0)
        } else {
            incoming
        }
    }
}

impl Default for Crunch {
    fn default() -> Self {
        Self::new(0.75)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_not_braced() {
        let c = Crunch::new(0.75);
        assert!(!c.is_braced());
        assert!(!c.just_braced);
    }

    #[test]
    fn crunch_activates_stance() {
        let mut c = Crunch::new(0.75);
        c.crunch();
        assert!(c.is_braced());
        assert!(c.just_braced);
    }

    #[test]
    fn crunch_no_op_when_already_braced() {
        let mut c = Crunch::new(0.75);
        c.crunch();
        c.tick();
        c.crunch();
        assert!(!c.just_braced);
    }

    #[test]
    fn crunch_no_op_when_disabled() {
        let mut c = Crunch::new(0.75);
        c.enabled = false;
        c.crunch();
        assert!(!c.active);
    }

    #[test]
    fn stand_exits_stance() {
        let mut c = Crunch::new(0.75);
        c.crunch();
        c.stand();
        assert!(!c.is_braced());
        assert!(c.just_broke);
    }

    #[test]
    fn stand_no_op_when_not_braced() {
        let mut c = Crunch::new(0.75);
        c.stand();
        assert!(!c.just_broke);
    }

    #[test]
    fn tick_clears_just_braced() {
        let mut c = Crunch::new(0.75);
        c.crunch();
        c.tick();
        assert!(!c.just_braced);
    }

    #[test]
    fn tick_clears_just_broke() {
        let mut c = Crunch::new(0.75);
        c.crunch();
        c.stand();
        c.tick();
        assert!(!c.just_broke);
    }

    #[test]
    fn is_braced_false_when_disabled() {
        let mut c = Crunch::new(0.75);
        c.crunch();
        c.enabled = false;
        assert!(!c.is_braced());
    }

    #[test]
    fn damage_taken_reduced_while_braced() {
        let mut c = Crunch::new(0.75);
        c.crunch();
        // 100 * (1 - 0.75) = 25
        assert!((c.damage_taken(100.0) - 25.0).abs() < 1e-3);
    }

    #[test]
    fn damage_taken_full_when_not_braced() {
        let c = Crunch::new(0.75);
        assert!((c.damage_taken(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn damage_taken_full_reduction_is_zero() {
        let mut c = Crunch::new(1.0);
        c.crunch();
        assert!((c.damage_taken(100.0)).abs() < 1e-5);
    }

    #[test]
    fn damage_taken_floored_at_zero() {
        let mut c = Crunch::new(1.0);
        c.crunch();
        assert!((c.damage_taken(50.0)).abs() < 1e-5);
    }

    #[test]
    fn damage_taken_full_when_disabled() {
        let mut c = Crunch::new(0.75);
        c.crunch();
        c.enabled = false;
        assert!((c.damage_taken(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn damage_reduction_clamped_to_one() {
        let c = Crunch::new(2.0);
        assert!((c.damage_reduction - 1.0).abs() < 1e-5);
    }

    #[test]
    fn damage_reduction_clamped_at_zero() {
        let c = Crunch::new(-0.5);
        assert!((c.damage_reduction).abs() < 1e-5);
    }

    #[test]
    fn partial_reduction() {
        let mut c = Crunch::new(0.5);
        c.crunch();
        // 80 * (1 - 0.5) = 40
        assert!((c.damage_taken(80.0) - 40.0).abs() < 1e-3);
    }

    #[test]
    fn can_re_crunch_after_standing() {
        let mut c = Crunch::new(0.75);
        c.crunch();
        c.stand();
        c.tick();
        c.crunch();
        assert!(c.is_braced());
        assert!(c.just_braced);
    }
}
