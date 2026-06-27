use bevy_ecs::prelude::Component;

/// Passive concealment that activates when the entity stands still. While
/// not moving, `camo_timer` accumulates. Once it reaches `activate_threshold`
/// the entity enters `camouflaged` — enemies have reduced ability to detect
/// it, modelled by `visibility_fraction()` approaching 0.0. Any movement
/// immediately breaks camouflage via `on_moved()`.
///
/// `tick(dt)` clears one-frame flags first; if enabled, increments
/// `camo_timer` (capped at `activate_threshold`); fires `just_hidden` on
/// the tick the timer first reaches the threshold. No-op when disabled.
///
/// `on_moved()` resets `camo_timer` to 0.0 and sets `camouflaged` to
/// `false`; fires `just_revealed` if the entity was camouflaged. No-op
/// when disabled.
///
/// `is_camouflaged()` returns `camouflaged && enabled`.
///
/// `visibility_fraction()` returns `1.0 - (camo_timer / activate_threshold).clamp(0, 1)`:
/// 1.0 (fully visible) when standing still and not yet hidden; 0.0 (fully
/// concealed) at `activate_threshold`.
///
/// Distinct from `Stealth` (active, intentional stealth mode that the
/// entity deliberately activates), `Ghost` (physical phase-through),
/// `Shroud` (area-of-effect obscurement), and `Dissolve` (visual fade with
/// no detection mechanic): Camouflage is a **passive stillness-based
/// concealment** — the entity blends into its environment automatically
/// while motionless and is instantly revealed the moment it moves.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Camouflage {
    /// Accumulated stillness time [0.0, activate_threshold].
    pub camo_timer: f32,
    /// Seconds of stillness required to become fully camouflaged. Clamped ≥ 0.1.
    pub activate_threshold: f32,
    pub camouflaged: bool,
    pub just_hidden: bool,
    pub just_revealed: bool,
    pub enabled: bool,
}

impl Camouflage {
    pub fn new(activate_threshold: f32) -> Self {
        Self {
            camo_timer: 0.0,
            activate_threshold: activate_threshold.max(0.1),
            camouflaged: false,
            just_hidden: false,
            just_revealed: false,
            enabled: true,
        }
    }

    /// Advance stillness timer. Clears `just_hidden` and `just_revealed`
    /// first; increments `camo_timer` (capped at `activate_threshold`);
    /// fires `just_hidden` on the first tick that reaches the threshold.
    /// No-op when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_hidden = false;
        self.just_revealed = false;

        if !self.enabled {
            return;
        }

        if !self.camouflaged {
            let was_below = self.camo_timer < self.activate_threshold;
            self.camo_timer = (self.camo_timer + dt).min(self.activate_threshold);
            if was_below && self.camo_timer >= self.activate_threshold {
                self.camouflaged = true;
                self.just_hidden = true;
            }
        }
    }

    /// Register movement. Resets `camo_timer` to 0.0 and exits camouflage.
    /// Fires `just_revealed` if the entity was camouflaged. No-op when
    /// disabled.
    pub fn on_moved(&mut self) {
        if !self.enabled {
            return;
        }
        if self.camouflaged {
            self.camouflaged = false;
            self.just_revealed = true;
        }
        self.camo_timer = 0.0;
    }

    /// `true` when the entity is fully camouflaged and the component is
    /// enabled.
    pub fn is_camouflaged(&self) -> bool {
        self.camouflaged && self.enabled
    }

    /// Visibility fraction [1.0 = fully visible, 0.0 = fully concealed].
    /// Decreases linearly as `camo_timer` approaches `activate_threshold`.
    pub fn visibility_fraction(&self) -> f32 {
        1.0 - (self.camo_timer / self.activate_threshold).clamp(0.0, 1.0)
    }
}

impl Default for Camouflage {
    fn default() -> Self {
        Self::new(3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_visible_and_not_camouflaged() {
        let c = Camouflage::new(3.0);
        assert_eq!(c.camo_timer, 0.0);
        assert!(!c.camouflaged);
        assert!(!c.is_camouflaged());
    }

    #[test]
    fn tick_increments_camo_timer() {
        let mut c = Camouflage::new(3.0);
        c.tick(1.0);
        assert!((c.camo_timer - 1.0).abs() < 1e-5);
    }

    #[test]
    fn tick_caps_at_activate_threshold() {
        let mut c = Camouflage::new(3.0);
        c.tick(100.0);
        assert!((c.camo_timer - 3.0).abs() < 1e-5);
    }

    #[test]
    fn tick_sets_camouflaged_at_threshold() {
        let mut c = Camouflage::new(2.0);
        c.tick(2.0);
        assert!(c.camouflaged);
        assert!(c.is_camouflaged());
    }

    #[test]
    fn tick_fires_just_hidden_on_first_activation() {
        let mut c = Camouflage::new(1.0);
        c.tick(1.0);
        assert!(c.just_hidden);
    }

    #[test]
    fn tick_no_just_hidden_when_already_camouflaged() {
        let mut c = Camouflage::new(1.0);
        c.tick(1.0); // just_hidden fires
        c.tick(0.016); // already camouflaged, flag cleared
        assert!(!c.just_hidden);
    }

    #[test]
    fn tick_timer_stops_growing_once_camouflaged() {
        let mut c = Camouflage::new(2.0);
        c.tick(2.0); // reaches threshold
        c.tick(5.0); // should not grow past threshold
        assert!((c.camo_timer - 2.0).abs() < 1e-5);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut c = Camouflage::new(2.0);
        c.enabled = false;
        c.tick(5.0);
        assert_eq!(c.camo_timer, 0.0);
        assert!(!c.camouflaged);
    }

    #[test]
    fn tick_clears_just_hidden_each_frame() {
        let mut c = Camouflage::new(1.0);
        c.tick(1.0); // just_hidden = true
        c.tick(0.016); // cleared
        assert!(!c.just_hidden);
    }

    #[test]
    fn tick_clears_just_revealed_each_frame() {
        let mut c = Camouflage::new(1.0);
        c.tick(1.0); // camouflaged
        c.on_moved(); // just_revealed = true
        c.tick(0.016); // cleared
        assert!(!c.just_revealed);
    }

    #[test]
    fn on_moved_resets_timer() {
        let mut c = Camouflage::new(3.0);
        c.tick(1.5);
        c.on_moved();
        assert_eq!(c.camo_timer, 0.0);
    }

    #[test]
    fn on_moved_clears_camouflaged() {
        let mut c = Camouflage::new(1.0);
        c.tick(1.0); // camouflaged
        c.on_moved();
        assert!(!c.camouflaged);
        assert!(!c.is_camouflaged());
    }

    #[test]
    fn on_moved_fires_just_revealed_when_camouflaged() {
        let mut c = Camouflage::new(1.0);
        c.tick(1.0); // camouflaged
        c.on_moved();
        assert!(c.just_revealed);
    }

    #[test]
    fn on_moved_no_just_revealed_when_not_camouflaged() {
        let mut c = Camouflage::new(3.0);
        c.tick(1.0); // not yet camouflaged
        c.on_moved();
        assert!(!c.just_revealed);
        assert_eq!(c.camo_timer, 0.0);
    }

    #[test]
    fn on_moved_no_op_when_disabled() {
        let mut c = Camouflage::new(1.0);
        c.tick(1.0); // camouflaged
        c.enabled = false;
        c.on_moved();
        assert!(c.camouflaged); // state unchanged
        assert!((c.camo_timer - 1.0).abs() < 1e-5);
    }

    #[test]
    fn is_camouflaged_false_when_disabled() {
        let mut c = Camouflage::new(1.0);
        c.camouflaged = true;
        c.enabled = false;
        assert!(!c.is_camouflaged());
    }

    #[test]
    fn visibility_fraction_one_at_start() {
        let c = Camouflage::new(3.0);
        assert!((c.visibility_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn visibility_fraction_half_at_mid_timer() {
        let mut c = Camouflage::new(4.0);
        c.tick(2.0); // 2/4 = 0.5 → visibility = 1 - 0.5 = 0.5
        assert!((c.visibility_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn visibility_fraction_zero_at_full_camo() {
        let mut c = Camouflage::new(2.0);
        c.tick(2.0);
        assert!((c.visibility_fraction()).abs() < 1e-5);
    }

    #[test]
    fn visibility_fraction_one_after_move_breaks_camo() {
        let mut c = Camouflage::new(1.0);
        c.tick(1.0); // fully hidden
        c.on_moved(); // timer resets
        assert!((c.visibility_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn re_camouflages_after_moving_and_waiting() {
        let mut c = Camouflage::new(1.0);
        c.tick(1.0); // camouflaged
        c.tick(0.016);
        c.on_moved(); // broken
        c.tick(0.016);
        c.tick(1.0); // camouflaged again
        assert!(c.camouflaged);
        assert!(c.just_hidden);
    }

    #[test]
    fn partial_timer_builds_up_correctly() {
        let mut c = Camouflage::new(3.0);
        c.tick(1.0);
        c.tick(1.0);
        c.tick(1.0);
        assert!(c.camouflaged);
    }

    #[test]
    fn just_revealed_fires_only_on_transition() {
        let mut c = Camouflage::new(1.0);
        c.tick(1.0); // camouflaged
        c.on_moved(); // just_revealed = true
                      // on_moved again (no longer camouflaged) — just_revealed should NOT fire again
        c.tick(0.016); // clear flags
        c.on_moved();
        assert!(!c.just_revealed);
    }

    #[test]
    fn activate_threshold_clamped_to_minimum() {
        let c = Camouflage::new(0.0);
        assert!((c.activate_threshold - 0.1).abs() < 1e-5);
    }

    #[test]
    fn on_moved_during_inactive_timer_still_resets() {
        let mut c = Camouflage::new(3.0);
        c.tick(2.0);
        c.on_moved();
        assert_eq!(c.camo_timer, 0.0);
    }
}
