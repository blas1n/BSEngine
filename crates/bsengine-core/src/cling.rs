use bevy_ecs::prelude::Component;

/// Wall-stick mechanic: while clinging the entity adheres to any surface
/// (wall, ceiling, sloped terrain) and gravity is negated by the physics
/// system. Cling can be indefinite (`max_duration == 0.0`) or automatically
/// release after `max_duration` seconds.
///
/// `cling()` latches the entity to a surface, fires `just_clung`, and — when
/// `max_duration > 0` — starts a countdown. `release()` detaches and fires
/// `just_released`. `tick(dt)` counts down and auto-releases on expiry.
///
/// `cling()` is a no-op when already clinging or when disabled.
/// `release()` is a no-op when not clinging.
///
/// Distinct from `Climb` (active locomotion along a surface), `Hover` (aerial
/// float), and `Grab` (gripping another entity): Cling is the **wall-stick**
/// mechanic — the entity passively adheres to the surface it touches and holds
/// position without moving, until it actively releases or the timer expires.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Cling {
    pub active: bool,
    /// Auto-release duration in seconds. `0.0` means indefinite — the cling
    /// persists until `release()` is called. Clamped ≥ 0.0.
    pub max_duration: f32,
    /// Remaining countdown in seconds. Only meaningful when `max_duration > 0`.
    pub cling_timer: f32,
    pub just_clung: bool,
    pub just_released: bool,
    pub enabled: bool,
}

impl Cling {
    /// Create a Cling component. `max_duration == 0.0` → indefinite;
    /// `max_duration > 0.0` → auto-release after that many seconds.
    pub fn new(max_duration: f32) -> Self {
        Self {
            active: false,
            max_duration: max_duration.max(0.0),
            cling_timer: 0.0,
            just_clung: false,
            just_released: false,
            enabled: true,
        }
    }

    /// Latch on to a surface. Fires `just_clung`. Starts the countdown when
    /// `max_duration > 0`. No-op when already clinging or disabled.
    pub fn cling(&mut self) {
        if self.active || !self.enabled {
            return;
        }
        self.active = true;
        self.just_clung = true;
        if self.max_duration > 0.0 {
            self.cling_timer = self.max_duration;
        }
    }

    /// Detach from the surface. Fires `just_released`. No-op when not clinging.
    pub fn release(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        self.cling_timer = 0.0;
        self.just_released = true;
    }

    /// Advance the countdown when `max_duration > 0`. Auto-releases and fires
    /// `just_released` on expiry. Clears one-frame flags at the start of each
    /// tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_clung = false;
        self.just_released = false;

        if self.active && self.max_duration > 0.0 && self.cling_timer > 0.0 {
            self.cling_timer -= dt;
            if self.cling_timer <= 0.0 {
                self.cling_timer = 0.0;
                self.active = false;
                self.just_released = true;
            }
        }
    }

    /// `true` while latched to a surface and enabled.
    pub fn is_clinging(&self) -> bool {
        self.active && self.enabled
    }

    /// Fraction of the cling duration remaining [1.0 = just latched, 0.0 =
    /// about to auto-release]. Returns 1.0 when `max_duration == 0` (indefinite)
    /// or when not clinging.
    pub fn time_fraction(&self) -> f32 {
        if self.max_duration <= 0.0 || !self.active {
            return 1.0;
        }
        (self.cling_timer / self.max_duration).clamp(0.0, 1.0)
    }
}

impl Default for Cling {
    fn default() -> Self {
        Self::new(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_not_clinging() {
        let c = Cling::new(0.0);
        assert!(!c.is_clinging());
        assert!(!c.just_clung);
    }

    #[test]
    fn cling_activates_indefinite() {
        let mut c = Cling::new(0.0);
        c.cling();
        assert!(c.is_clinging());
        assert!(c.just_clung);
        assert_eq!(c.cling_timer, 0.0); // timer unused when indefinite
    }

    #[test]
    fn cling_activates_timed_and_sets_timer() {
        let mut c = Cling::new(3.0);
        c.cling();
        assert!(c.is_clinging());
        assert!(c.just_clung);
        assert!((c.cling_timer - 3.0).abs() < 1e-5);
    }

    #[test]
    fn cling_no_op_when_already_clinging() {
        let mut c = Cling::new(3.0);
        c.cling();
        c.tick(0.016);
        c.cling(); // already active
        assert!(!c.just_clung);
    }

    #[test]
    fn cling_no_op_when_disabled() {
        let mut c = Cling::new(0.0);
        c.enabled = false;
        c.cling();
        assert!(!c.active);
        assert!(!c.just_clung);
    }

    #[test]
    fn release_detaches() {
        let mut c = Cling::new(0.0);
        c.cling();
        c.release();
        assert!(!c.is_clinging());
        assert!(c.just_released);
        assert_eq!(c.cling_timer, 0.0);
    }

    #[test]
    fn release_no_op_when_not_clinging() {
        let mut c = Cling::new(0.0);
        c.release(); // no panic
        assert!(!c.just_released);
    }

    #[test]
    fn tick_counts_down_timed_cling() {
        let mut c = Cling::new(5.0);
        c.cling();
        c.tick(2.0);
        assert!((c.cling_timer - 3.0).abs() < 1e-3);
        assert!(c.is_clinging());
    }

    #[test]
    fn tick_auto_releases_on_expiry() {
        let mut c = Cling::new(2.0);
        c.cling();
        c.tick(2.5);
        assert!(!c.is_clinging());
        assert!(c.just_released);
        assert_eq!(c.cling_timer, 0.0);
    }

    #[test]
    fn tick_no_countdown_when_indefinite() {
        let mut c = Cling::new(0.0);
        c.cling();
        c.tick(100.0);
        assert!(c.is_clinging()); // still clinging
        assert_eq!(c.cling_timer, 0.0);
    }

    #[test]
    fn tick_clears_just_clung() {
        let mut c = Cling::new(0.0);
        c.cling();
        c.tick(0.016);
        assert!(!c.just_clung);
    }

    #[test]
    fn tick_clears_just_released() {
        let mut c = Cling::new(1.0);
        c.cling();
        c.tick(2.0); // expires
        c.tick(0.016);
        assert!(!c.just_released);
    }

    #[test]
    fn is_clinging_false_when_disabled() {
        let mut c = Cling::new(0.0);
        c.cling();
        c.enabled = false;
        assert!(!c.is_clinging());
    }

    #[test]
    fn time_fraction_one_when_indefinite() {
        let mut c = Cling::new(0.0);
        c.cling();
        assert!((c.time_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn time_fraction_one_at_start() {
        let mut c = Cling::new(4.0);
        c.cling();
        assert!((c.time_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn time_fraction_at_half() {
        let mut c = Cling::new(4.0);
        c.cling();
        c.tick(2.0);
        assert!((c.time_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn time_fraction_one_when_not_clinging() {
        let c = Cling::new(4.0);
        assert!((c.time_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn can_re_cling_after_release() {
        let mut c = Cling::new(3.0);
        c.cling();
        c.release();
        c.tick(0.016);
        c.cling();
        assert!(c.is_clinging());
        assert!(c.just_clung);
        assert!((c.cling_timer - 3.0).abs() < 1e-5);
    }

    #[test]
    fn can_re_cling_after_expiry() {
        let mut c = Cling::new(1.0);
        c.cling();
        c.tick(2.0); // expires
        c.tick(0.016); // clear flags
        c.cling();
        assert!(c.is_clinging());
        assert!(c.just_clung);
    }
}
