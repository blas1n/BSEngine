use bevy_ecs::prelude::Component;

/// Active-cooldown ranged energy discharge. Models instant-fire attacks
/// (lightning bolt, stun gun, railgun) that discharge on demand and cannot
/// refire until a cooldown elapses.
///
/// Distinct from `Wow` (lingering display that fades over duration) and
/// `Cooldown` (generic gate): Zap has **two** effective outputs — power and
/// range — and fires with no active duration, only a refire window.
///
/// `fire()` discharges if `enabled` and `cooldown_timer == 0`. Sets
/// `cooldown_timer = cooldown_duration` and `just_zapped = true`. No-op when
/// on cooldown or disabled.
///
/// `tick(dt)` clears `just_zapped`, then if enabled and `cooldown_timer > 0`
/// counts down toward 0. No-op (beyond flag clear) when disabled.
///
/// `is_ready()` returns `cooldown_timer == 0 && enabled`.
///
/// `readiness_fraction()` returns progress toward next ready state: `1.0 -
/// (cooldown_timer / cooldown_duration).clamp(0, 1)`. Returns `1.0` when
/// `cooldown_duration == 0` (instant refire, always ready).
///
/// `effective_power(base)` returns `base * zap_power` when enabled; `0.0`
/// when disabled.
///
/// `effective_range()` returns `zap_range` when enabled; `0.0` when
/// disabled.
///
/// Default: `new(1.0, 10.0, 1.0)` — power×1, range 10, 1-second cooldown.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zap {
    /// Discharge power multiplier. Clamped >= 0.0.
    pub zap_power: f32,
    /// Effective range of the discharge. Clamped >= 0.0.
    pub zap_range: f32,
    /// Cooldown between discharges in seconds. Clamped >= 0.0.
    pub cooldown_duration: f32,
    /// Remaining cooldown [0, cooldown_duration].
    pub cooldown_timer: f32,
    pub just_zapped: bool,
    pub enabled: bool,
}

impl Zap {
    pub fn new(zap_power: f32, zap_range: f32, cooldown_duration: f32) -> Self {
        Self {
            zap_power: zap_power.max(0.0),
            zap_range: zap_range.max(0.0),
            cooldown_duration: cooldown_duration.max(0.0),
            cooldown_timer: 0.0,
            just_zapped: false,
            enabled: true,
        }
    }

    /// Fire a discharge. Sets cooldown and `just_zapped`. No-op when on
    /// cooldown or disabled.
    pub fn fire(&mut self) {
        if !self.enabled || self.cooldown_timer > 0.0 {
            return;
        }
        self.cooldown_timer = self.cooldown_duration;
        self.just_zapped = true;
    }

    /// Advance one frame: clear `just_zapped`, then count down cooldown.
    /// No-op beyond flag clear when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_zapped = false;

        if !self.enabled {
            return;
        }

        if self.cooldown_timer > 0.0 {
            self.cooldown_timer = (self.cooldown_timer - dt).max(0.0);
        }
    }

    /// `true` when the component is ready to fire and enabled.
    pub fn is_ready(&self) -> bool {
        self.cooldown_timer == 0.0 && self.enabled
    }

    /// Cooldown progress toward ready [0.0=just fired, 1.0=fully ready].
    /// Always `1.0` when `cooldown_duration == 0` (instant refire).
    pub fn readiness_fraction(&self) -> f32 {
        if self.cooldown_duration == 0.0 {
            return 1.0;
        }
        (1.0 - self.cooldown_timer / self.cooldown_duration).clamp(0.0, 1.0)
    }

    /// Discharge power scaled from `base`. Returns `base * zap_power` when
    /// enabled; `0.0` when disabled.
    pub fn effective_power(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.zap_power
    }

    /// Effective discharge range. Returns `zap_range` when enabled; `0.0`
    /// when disabled.
    pub fn effective_range(&self) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        self.zap_range
    }
}

impl Default for Zap {
    fn default() -> Self {
        Self::new(1.0, 10.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zap {
        Zap::new(2.0, 15.0, 1.0) // power×2, range=15, cooldown=1s
    }

    // --- construction ---

    #[test]
    fn new_starts_ready() {
        let z = z();
        assert_eq!(z.cooldown_timer, 0.0);
        assert!(!z.just_zapped);
        assert!(z.is_ready());
    }

    #[test]
    fn zap_power_clamped_to_zero() {
        let z = Zap::new(-1.0, 10.0, 1.0);
        assert_eq!(z.zap_power, 0.0);
    }

    #[test]
    fn zap_range_clamped_to_zero() {
        let z = Zap::new(1.0, -5.0, 1.0);
        assert_eq!(z.zap_range, 0.0);
    }

    #[test]
    fn cooldown_duration_clamped_to_zero() {
        let z = Zap::new(1.0, 10.0, -2.0);
        assert_eq!(z.cooldown_duration, 0.0);
    }

    // --- fire ---

    #[test]
    fn fire_sets_just_zapped() {
        let mut z = z();
        z.fire();
        assert!(z.just_zapped);
    }

    #[test]
    fn fire_sets_cooldown_timer() {
        let mut z = z();
        z.fire();
        assert!((z.cooldown_timer - 1.0).abs() < 1e-5);
    }

    #[test]
    fn fire_no_op_on_cooldown() {
        let mut z = z();
        z.fire(); // starts cooldown
        z.just_zapped = false; // manually clear
        z.fire(); // on cooldown — no-op
        assert!(!z.just_zapped);
    }

    #[test]
    fn fire_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.fire();
        assert!(!z.just_zapped);
        assert_eq!(z.cooldown_timer, 0.0);
    }

    #[test]
    fn fire_zero_cooldown_sets_timer_to_zero() {
        let mut z = Zap::new(1.0, 10.0, 0.0);
        z.fire();
        assert!(z.just_zapped);
        assert_eq!(z.cooldown_timer, 0.0); // still 0 — instant refire
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_zapped() {
        let mut z = z();
        z.fire();
        z.tick(0.016);
        assert!(!z.just_zapped);
    }

    #[test]
    fn tick_counts_down_cooldown() {
        let mut z = z();
        z.fire();
        z.tick(0.5);
        assert!((z.cooldown_timer - 0.5).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_cooldown_fully() {
        let mut z = z();
        z.fire();
        z.tick(1.5); // overshoots
        assert_eq!(z.cooldown_timer, 0.0);
    }

    #[test]
    fn tick_no_op_cooldown_when_disabled() {
        let mut z = z();
        z.fire();
        z.enabled = false;
        z.tick(1.0);
        // timer stays at 1.0 since disabled
        assert!((z.cooldown_timer - 1.0).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_just_zapped_when_disabled() {
        let mut z = z();
        z.just_zapped = true;
        z.enabled = false;
        z.tick(0.016);
        assert!(!z.just_zapped);
    }

    #[test]
    fn tick_no_change_when_ready_and_enabled() {
        let mut z = z();
        z.tick(1.0); // already at 0, nothing to do
        assert_eq!(z.cooldown_timer, 0.0);
        assert!(!z.just_zapped);
    }

    // --- is_ready ---

    #[test]
    fn is_ready_true_at_start() {
        let z = z();
        assert!(z.is_ready());
    }

    #[test]
    fn is_ready_false_during_cooldown() {
        let mut z = z();
        z.fire();
        assert!(!z.is_ready());
    }

    #[test]
    fn is_ready_true_after_cooldown_expires() {
        let mut z = z();
        z.fire();
        z.tick(1.0);
        assert!(z.is_ready());
    }

    #[test]
    fn is_ready_false_when_disabled() {
        let z_dis = {
            let mut z = z();
            z.enabled = false;
            z
        };
        assert!(!z_dis.is_ready());
    }

    // --- readiness_fraction ---

    #[test]
    fn readiness_fraction_one_when_ready() {
        let z = z();
        assert!((z.readiness_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn readiness_fraction_zero_just_after_fire() {
        let mut z = z();
        z.fire();
        assert!((z.readiness_fraction() - 0.0).abs() < 1e-5);
    }

    #[test]
    fn readiness_fraction_half_at_midpoint() {
        let mut z = z(); // cooldown=1.0
        z.fire();
        z.tick(0.5); // 0.5 elapsed
        assert!((z.readiness_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn readiness_fraction_one_for_zero_cooldown() {
        let z = Zap::new(1.0, 10.0, 0.0);
        assert!((z.readiness_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn readiness_fraction_one_for_zero_cooldown_after_fire() {
        let mut z = Zap::new(1.0, 10.0, 0.0);
        z.fire();
        assert!((z.readiness_fraction() - 1.0).abs() < 1e-5);
    }

    // --- effective_power ---

    #[test]
    fn effective_power_scales_by_zap_power() {
        let z = z(); // power=2.0 → 100*2=200
        assert!((z.effective_power(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_power_zero_when_disabled() {
        let mut z = z();
        z.enabled = false;
        assert_eq!(z.effective_power(100.0), 0.0);
    }

    #[test]
    fn effective_power_zero_at_zero_power() {
        let z = Zap::new(0.0, 10.0, 1.0);
        assert_eq!(z.effective_power(100.0), 0.0);
    }

    // --- effective_range ---

    #[test]
    fn effective_range_returns_zap_range() {
        let z = z(); // range=15
        assert!((z.effective_range() - 15.0).abs() < 1e-5);
    }

    #[test]
    fn effective_range_zero_when_disabled() {
        let mut z = z();
        z.enabled = false;
        assert_eq!(z.effective_range(), 0.0);
    }

    #[test]
    fn effective_range_zero_at_zero_range() {
        let z = Zap::new(1.0, 0.0, 1.0);
        assert_eq!(z.effective_range(), 0.0);
    }

    // --- fire/tick cycle ---

    #[test]
    fn can_refire_after_cooldown() {
        let mut z = z();
        z.fire();
        z.tick(1.0); // cooldown expires
        z.tick(0.0); // clear flags
        z.fire(); // second shot
        assert!(z.just_zapped);
        assert!((z.cooldown_timer - 1.0).abs() < 1e-5);
    }

    #[test]
    fn zero_cooldown_allows_every_frame_fire() {
        let mut z = Zap::new(1.0, 10.0, 0.0);
        z.fire();
        z.tick(0.016);
        z.fire(); // immediately ready again
        assert!(z.just_zapped);
    }
}
