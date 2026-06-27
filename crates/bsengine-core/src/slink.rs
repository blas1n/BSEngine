use bevy_ecs::prelude::Component;

/// Noise-suppression movement trade: while `active`, the entity moves at
/// `base * (1 - speed_reduction)` but generates only `base * (1 - noise_reduction)`
/// of its normal audio/footstep signature. Slink can be toggled on and off
/// at any time; transitioning fires one-frame event flags.
///
/// `engage()` activates slink and sets `just_engaged` on the inactive →
/// active transition. `disengage()` deactivates and sets `just_disengaged`.
/// `tick()` clears one-frame flags.
///
/// `effective_speed(base)` returns the slowed value while active and enabled;
/// `effective_noise(base)` returns the reduced noise value. Both return `base`
/// when inactive or disabled.
///
/// Distinct from `Stealth` (full-invisibility timed window), `Ghost` (passes
/// through solid geometry), and `Prowl` (heightened perception while skulking):
/// Slink is a **noise-for-speed trade** — a toggle that suppresses audio
/// footprint in exchange for slower movement. No timer; no cooldown; purely
/// mechanical on/off.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Slink {
    /// Whether the entity is currently slinking. Set by `engage`/`disengage`.
    pub active: bool,
    /// Fraction of movement speed lost while active. Clamped [0.0, 1.0].
    pub speed_reduction: f32,
    /// Fraction of noise suppressed while active. Clamped [0.0, 1.0].
    pub noise_reduction: f32,
    pub just_engaged: bool,
    pub just_disengaged: bool,
    pub enabled: bool,
}

impl Slink {
    pub fn new(speed_reduction: f32, noise_reduction: f32) -> Self {
        Self {
            active: false,
            speed_reduction: speed_reduction.clamp(0.0, 1.0),
            noise_reduction: noise_reduction.clamp(0.0, 1.0),
            just_engaged: false,
            just_disengaged: false,
            enabled: true,
        }
    }

    /// Activate slink. Sets `just_engaged` on the inactive → active
    /// transition. No-op when already active or disabled.
    pub fn engage(&mut self) {
        if !self.enabled || self.active {
            return;
        }
        self.active = true;
        self.just_engaged = true;
    }

    /// Deactivate slink. Sets `just_disengaged` on the active → inactive
    /// transition. No-op when already inactive.
    pub fn disengage(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        self.just_disengaged = true;
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_engaged = false;
        self.just_disengaged = false;
    }

    /// Effective movement speed: `base * (1 - speed_reduction)` when active
    /// and enabled, floored at `0.0`. Returns `base` otherwise.
    pub fn effective_speed(&self, base: f32) -> f32 {
        if self.active && self.enabled {
            (base * (1.0 - self.speed_reduction)).max(0.0)
        } else {
            base
        }
    }

    /// Effective noise output: `base * (1 - noise_reduction)` when active
    /// and enabled, floored at `0.0`. Returns `base` otherwise.
    pub fn effective_noise(&self, base: f32) -> f32 {
        if self.active && self.enabled {
            (base * (1.0 - self.noise_reduction)).max(0.0)
        } else {
            base
        }
    }

    pub fn is_slinking(&self) -> bool {
        self.active && self.enabled
    }
}

impl Default for Slink {
    fn default() -> Self {
        Self::new(0.3, 0.7)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engage_activates_and_sets_flag() {
        let mut s = Slink::new(0.3, 0.7);
        s.engage();
        assert!(s.active);
        assert!(s.just_engaged);
        assert!(!s.just_disengaged);
    }

    #[test]
    fn engage_no_op_when_already_active() {
        let mut s = Slink::new(0.3, 0.7);
        s.engage();
        s.tick();
        s.engage();
        assert!(!s.just_engaged);
    }

    #[test]
    fn engage_no_op_when_disabled() {
        let mut s = Slink::new(0.3, 0.7);
        s.enabled = false;
        s.engage();
        assert!(!s.active);
        assert!(!s.just_engaged);
    }

    #[test]
    fn disengage_deactivates_and_sets_flag() {
        let mut s = Slink::new(0.3, 0.7);
        s.engage();
        s.tick();
        s.disengage();
        assert!(!s.active);
        assert!(s.just_disengaged);
        assert!(!s.just_engaged);
    }

    #[test]
    fn disengage_no_op_when_already_inactive() {
        let mut s = Slink::new(0.3, 0.7);
        s.disengage();
        assert!(!s.just_disengaged);
    }

    #[test]
    fn tick_clears_just_engaged() {
        let mut s = Slink::new(0.3, 0.7);
        s.engage();
        s.tick();
        assert!(!s.just_engaged);
    }

    #[test]
    fn tick_clears_just_disengaged() {
        let mut s = Slink::new(0.3, 0.7);
        s.engage();
        s.disengage();
        s.tick();
        assert!(!s.just_disengaged);
    }

    #[test]
    fn effective_speed_reduced_when_active() {
        let mut s = Slink::new(0.3, 0.7);
        s.engage();
        // 100 * (1 - 0.3) = 70
        assert!((s.effective_speed(100.0) - 70.0).abs() < 1e-3);
    }

    #[test]
    fn effective_speed_zero_reduction_unchanged() {
        let mut s = Slink::new(0.0, 0.5);
        s.engage();
        assert!((s.effective_speed(100.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_speed_full_reduction_zero() {
        let mut s = Slink::new(1.0, 0.5);
        s.engage();
        assert!((s.effective_speed(100.0)).abs() < 1e-5);
    }

    #[test]
    fn effective_speed_base_when_inactive() {
        let s = Slink::new(0.3, 0.7);
        assert!((s.effective_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_noise_reduced_when_active() {
        let mut s = Slink::new(0.3, 0.7);
        s.engage();
        // 100 * (1 - 0.7) = 30
        assert!((s.effective_noise(100.0) - 30.0).abs() < 1e-3);
    }

    #[test]
    fn effective_noise_base_when_inactive() {
        let s = Slink::new(0.3, 0.7);
        assert!((s.effective_noise(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_noise_full_reduction_zero() {
        let mut s = Slink::new(0.3, 1.0);
        s.engage();
        assert!((s.effective_noise(100.0)).abs() < 1e-5);
    }

    #[test]
    fn disabled_effective_speed_base() {
        let mut s = Slink::new(0.3, 0.7);
        s.engage();
        s.enabled = false;
        assert!((s.effective_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_effective_noise_base() {
        let mut s = Slink::new(0.3, 0.7);
        s.engage();
        s.enabled = false;
        assert!((s.effective_noise(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn is_slinking_requires_active_and_enabled() {
        let mut s = Slink::new(0.3, 0.7);
        assert!(!s.is_slinking());
        s.engage();
        assert!(s.is_slinking());
        s.enabled = false;
        assert!(!s.is_slinking());
    }

    #[test]
    fn engage_disengage_cycle() {
        let mut s = Slink::new(0.3, 0.7);
        s.engage();
        assert!(s.is_slinking());
        s.disengage();
        assert!(!s.is_slinking());
        s.tick();
        s.engage();
        assert!(s.is_slinking());
    }
}
