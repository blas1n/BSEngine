use bevy_ecs::prelude::Component;

/// Forced-targeting debuff: while goaded, the entity is compelled to direct
/// all attacks at the goading source. The combat system reads `is_goaded()`
/// each frame and overrides target selection accordingly.
///
/// `goad(duration)` activates the compulsion for `duration` seconds
/// (high-watermark: only extends when the new duration exceeds the current
/// timer). `cleanse()` removes the effect early. `tick(dt)` counts down and
/// fires `just_freed` on natural expiry.
///
/// One-frame flags `just_goaded` and `just_freed` are cleared at the start
/// of each `tick()` call.
///
/// Distinct from `Taunt` (the active component on the entity applying the
/// effect), `Provoke` (entity self-invites aggro without forced targeting),
/// and `Fear` (panic response that causes flee behavior): Goad is the
/// **forced-targeting debuff on the recipient** — the entity cannot choose
/// any target other than the one that goaded it.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Goad {
    pub active: bool,
    /// Remaining forced-targeting duration in seconds.
    pub timer: f32,
    pub just_goaded: bool,
    pub just_freed: bool,
    pub enabled: bool,
}

impl Goad {
    pub fn new() -> Self {
        Self {
            active: false,
            timer: 0.0,
            just_goaded: false,
            just_freed: false,
            enabled: true,
        }
    }

    /// Apply or extend the forced-targeting effect for `duration` seconds.
    /// High-watermark: only replaces the current timer when `duration >
    /// timer`. Fires `just_goaded` on the inactive → active transition.
    /// No-op when disabled or `duration ≤ 0`.
    pub fn goad(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_goaded = self.active;
            self.timer = duration;
            self.active = true;
            if !was_goaded {
                self.just_goaded = true;
            }
        }
    }

    /// Remove the forced-targeting effect early. Fires `just_freed`.
    /// No-op when not currently goaded.
    pub fn cleanse(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        self.timer = 0.0;
        self.just_freed = true;
    }

    /// Advance the countdown. Fires `just_freed` when the timer reaches 0.
    /// Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_goaded = false;
        self.just_freed = false;

        if self.active && self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.active = false;
                self.just_freed = true;
            }
        }
    }

    /// `true` while the forced-targeting effect is active and enabled.
    pub fn is_goaded(&self) -> bool {
        self.active && self.enabled
    }
}

impl Default for Goad {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_not_goaded() {
        let g = Goad::new();
        assert!(!g.is_goaded());
        assert_eq!(g.timer, 0.0);
    }

    #[test]
    fn goad_activates() {
        let mut g = Goad::new();
        g.goad(3.0);
        assert!(g.is_goaded());
        assert!(g.just_goaded);
        assert!((g.timer - 3.0).abs() < 1e-5);
    }

    #[test]
    fn goad_extends_on_longer_duration() {
        let mut g = Goad::new();
        g.goad(2.0);
        g.tick(0.5);
        g.goad(5.0);
        assert!((g.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn goad_no_extend_on_shorter_duration() {
        let mut g = Goad::new();
        g.goad(5.0);
        g.goad(2.0);
        assert!((g.timer - 5.0).abs() < 1e-5);
    }

    #[test]
    fn just_goaded_not_set_on_extend() {
        let mut g = Goad::new();
        g.goad(2.0);
        g.tick(0.016);
        g.goad(5.0);
        assert!(!g.just_goaded);
    }

    #[test]
    fn goad_no_op_when_disabled() {
        let mut g = Goad::new();
        g.enabled = false;
        g.goad(3.0);
        assert!(!g.active);
    }

    #[test]
    fn goad_no_op_when_duration_zero() {
        let mut g = Goad::new();
        g.goad(0.0);
        assert!(!g.active);
    }

    #[test]
    fn goad_no_op_when_duration_negative() {
        let mut g = Goad::new();
        g.goad(-1.0);
        assert!(!g.active);
    }

    #[test]
    fn cleanse_removes_effect() {
        let mut g = Goad::new();
        g.goad(3.0);
        g.cleanse();
        assert!(!g.is_goaded());
        assert!(g.just_freed);
        assert_eq!(g.timer, 0.0);
    }

    #[test]
    fn cleanse_no_op_when_not_goaded() {
        let mut g = Goad::new();
        g.cleanse();
        assert!(!g.just_freed);
    }

    #[test]
    fn tick_counts_down() {
        let mut g = Goad::new();
        g.goad(5.0);
        g.tick(2.0);
        assert!((g.timer - 3.0).abs() < 1e-4);
        assert!(g.is_goaded());
    }

    #[test]
    fn tick_expires_effect() {
        let mut g = Goad::new();
        g.goad(2.0);
        g.tick(2.5);
        assert!(!g.is_goaded());
        assert!(g.just_freed);
        assert_eq!(g.timer, 0.0);
    }

    #[test]
    fn tick_clears_just_goaded() {
        let mut g = Goad::new();
        g.goad(3.0);
        g.tick(0.016);
        assert!(!g.just_goaded);
    }

    #[test]
    fn tick_clears_just_freed() {
        let mut g = Goad::new();
        g.goad(1.0);
        g.tick(2.0); // expires, sets just_freed
        g.tick(0.016);
        assert!(!g.just_freed);
    }

    #[test]
    fn is_goaded_false_when_disabled() {
        let mut g = Goad::new();
        g.goad(3.0);
        g.enabled = false;
        assert!(!g.is_goaded());
    }

    #[test]
    fn can_re_goad_after_cleanse() {
        let mut g = Goad::new();
        g.goad(2.0);
        g.cleanse();
        g.tick(0.016);
        g.goad(3.0);
        assert!(g.is_goaded());
        assert!(g.just_goaded);
    }

    #[test]
    fn can_re_goad_after_expiry() {
        let mut g = Goad::new();
        g.goad(1.0);
        g.tick(2.0); // expires
        g.tick(0.016); // clear flags
        g.goad(2.0);
        assert!(g.is_goaded());
        assert!(g.just_goaded);
    }
}
