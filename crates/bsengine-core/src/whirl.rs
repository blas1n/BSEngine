use bevy_ecs::prelude::Component;

use std::f32::consts::TAU;

/// Self-rotation tracker for spinning entities. While `active`, `angle`
/// advances at `spin_speed` radians per second; on each full revolution
/// (`angle >= 2π`) the angle wraps, `just_lapped` fires, and `revolutions`
/// increments. Systems can read `just_lapped` to apply per-revolution effects
/// (damage pulses, ability resets, visual flashes) and `revolution_fraction()`
/// to drive rotation visuals or sweep checks.
///
/// `spin_up()` activates spinning. No-op when already active or disabled.
///
/// `spin_down()` deactivates spinning. No-op when not active.
///
/// `tick(dt)` clears `just_lapped` first; when active: advances `angle` by
/// `spin_speed * dt`; fires `just_lapped` and increments `revolutions` each
/// time `angle` wraps past 2π (only one lap per tick even with large dt);
/// no-op when disabled.
///
/// `is_spinning()` returns `active && enabled`.
///
/// `revolution_fraction()` returns `angle / TAU` clamped to [0.0, 1.0] —
/// how far through the current revolution the entity is.
///
/// Distinct from `Orbit` (another object orbits the entity at arm's length),
/// `Angular_velocity` (raw physics rotational speed with no lap tracking),
/// `Whirlpool` (area-pull effect), and `Wisp` (a companion orbit with heal
/// pulses): Whirl tracks the **entity's own body rotation** and fires a
/// one-frame flag on each full lap for systems to hook into.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Whirl {
    /// Current rotation angle in radians, wrapped to [0, 2π).
    pub angle: f32,
    /// Rotation speed in radians per second. Clamped >= 0.0.
    pub spin_speed: f32,
    /// Total full revolutions completed since last `spin_up()`. Persists
    /// across `spin_down()` / `spin_up()` cycles.
    pub revolutions: u32,
    pub active: bool,
    pub just_lapped: bool,
    pub enabled: bool,
}

impl Whirl {
    pub fn new(spin_speed: f32) -> Self {
        Self {
            angle: 0.0,
            spin_speed: spin_speed.max(0.0),
            revolutions: 0,
            active: false,
            just_lapped: false,
            enabled: true,
        }
    }

    /// Start spinning. No-op when already active or disabled.
    pub fn spin_up(&mut self) {
        if !self.enabled || self.active {
            return;
        }
        self.active = true;
    }

    /// Stop spinning. No-op when not active.
    pub fn spin_down(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
    }

    /// Advance rotation. Clears `just_lapped` first; when active: increments
    /// `angle`; fires `just_lapped` and increments `revolutions` on each
    /// full revolution wrap (one wrap per tick regardless of dt). No-op when
    /// disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_lapped = false;

        if !self.enabled || !self.active {
            return;
        }

        self.angle += self.spin_speed * dt;
        if self.angle >= TAU {
            self.angle -= TAU;
            self.just_lapped = true;
            self.revolutions += 1;
        }
    }

    /// `true` when the entity is actively spinning and the component is
    /// enabled.
    pub fn is_spinning(&self) -> bool {
        self.active && self.enabled
    }

    /// Fraction of the current revolution completed [0.0, 1.0].
    pub fn revolution_fraction(&self) -> f32 {
        (self.angle / TAU).clamp(0.0, 1.0)
    }
}

impl Default for Whirl {
    fn default() -> Self {
        Self::new(3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_inactive() {
        let w = Whirl::new(3.0);
        assert!(!w.active);
        assert!(!w.is_spinning());
        assert_eq!(w.angle, 0.0);
        assert_eq!(w.revolutions, 0);
    }

    #[test]
    fn spin_up_sets_active() {
        let mut w = Whirl::new(3.0);
        w.spin_up();
        assert!(w.active);
        assert!(w.is_spinning());
    }

    #[test]
    fn spin_up_no_op_when_already_active() {
        let mut w = Whirl::new(3.0);
        w.spin_up();
        w.tick(1.0); // advance angle
        let angle_before = w.angle;
        w.spin_up(); // should not reset angle
        assert!((w.angle - angle_before).abs() < 1e-5);
    }

    #[test]
    fn spin_up_no_op_when_disabled() {
        let mut w = Whirl::new(3.0);
        w.enabled = false;
        w.spin_up();
        assert!(!w.active);
    }

    #[test]
    fn spin_down_clears_active() {
        let mut w = Whirl::new(3.0);
        w.spin_up();
        w.spin_down();
        assert!(!w.active);
    }

    #[test]
    fn spin_down_no_op_when_not_active() {
        let mut w = Whirl::new(3.0);
        w.spin_down(); // should not panic
        assert!(!w.active);
    }

    #[test]
    fn spin_down_preserves_angle() {
        let mut w = Whirl::new(3.0);
        w.spin_up();
        w.tick(0.5);
        let angle = w.angle;
        w.spin_down();
        assert!((w.angle - angle).abs() < 1e-5);
    }

    #[test]
    fn tick_advances_angle_while_active() {
        let mut w = Whirl::new(2.0);
        w.spin_up();
        w.tick(1.0); // +2.0 rad
        assert!((w.angle - 2.0).abs() < 1e-5);
    }

    #[test]
    fn tick_no_advance_when_inactive() {
        let mut w = Whirl::new(3.0);
        w.tick(1.0);
        assert_eq!(w.angle, 0.0);
    }

    #[test]
    fn tick_no_advance_when_disabled() {
        let mut w = Whirl::new(3.0);
        w.spin_up();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.angle, 0.0);
    }

    #[test]
    fn tick_fires_just_lapped_on_full_revolution() {
        let mut w = Whirl::new(TAU); // exactly one revolution per second
        w.spin_up();
        w.tick(1.0);
        assert!(w.just_lapped);
        assert_eq!(w.revolutions, 1);
    }

    #[test]
    fn tick_wraps_angle_after_full_revolution() {
        let mut w = Whirl::new(TAU); // 2π rad/s
        w.spin_up();
        w.tick(1.5); // 1.5 * 2π = 3π, wraps to π
                     // angle = 3π - 2π = π
        assert!((w.angle - std::f32::consts::PI).abs() < 1e-4);
    }

    #[test]
    fn tick_no_just_lapped_before_full_revolution() {
        let mut w = Whirl::new(1.0);
        w.spin_up();
        w.tick(1.0); // 1.0 < 2π
        assert!(!w.just_lapped);
    }

    #[test]
    fn tick_clears_just_lapped_next_frame() {
        let mut w = Whirl::new(TAU);
        w.spin_up();
        w.tick(1.0); // just_lapped = true
        w.tick(0.016); // cleared
        assert!(!w.just_lapped);
    }

    #[test]
    fn tick_only_one_lap_per_tick_even_with_large_dt() {
        let mut w = Whirl::new(TAU);
        w.spin_up();
        w.tick(5.0); // 5 revolutions worth, but only one lap fires
        assert!(w.just_lapped);
        assert_eq!(w.revolutions, 1);
    }

    #[test]
    fn tick_multiple_small_steps_accumulate_revolutions() {
        let mut w = Whirl::new(TAU); // 1 revolution/second
        w.spin_up();
        for _ in 0..5 {
            w.tick(1.0);
        }
        assert_eq!(w.revolutions, 5);
    }

    #[test]
    fn is_spinning_false_when_not_active() {
        let w = Whirl::new(3.0);
        assert!(!w.is_spinning());
    }

    #[test]
    fn is_spinning_false_when_disabled() {
        let mut w = Whirl::new(3.0);
        w.spin_up();
        w.enabled = false;
        assert!(!w.is_spinning());
    }

    #[test]
    fn revolution_fraction_zero_at_start() {
        let w = Whirl::new(3.0);
        assert_eq!(w.revolution_fraction(), 0.0);
    }

    #[test]
    fn revolution_fraction_half_at_pi() {
        let mut w = Whirl::new(3.0);
        w.spin_up();
        w.angle = std::f32::consts::PI;
        assert!((w.revolution_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn revolution_fraction_near_one_before_lap() {
        let mut w = Whirl::new(TAU * 0.99); // almost one rev per second
        w.spin_up();
        w.tick(1.0); // 0.99 * 2π
        assert!(w.revolution_fraction() < 1.0);
        assert!(w.revolution_fraction() > 0.98);
    }

    #[test]
    fn revolution_fraction_resets_after_lap() {
        let mut w = Whirl::new(TAU * 1.25); // 1.25 rev/sec
        w.spin_up();
        w.tick(1.0); // 1.25 revolutions → wraps to 0.25
                     // fraction = 0.25
        assert!((w.revolution_fraction() - 0.25).abs() < 1e-4);
    }

    #[test]
    fn revolutions_persist_across_spin_cycles() {
        let mut w = Whirl::new(TAU);
        w.spin_up();
        w.tick(1.0); // 1 rev
        w.spin_down();
        w.spin_up();
        w.tick(1.0); // 2 rev total
        assert_eq!(w.revolutions, 2);
    }

    #[test]
    fn spin_speed_clamped_to_zero() {
        let w = Whirl::new(-5.0);
        assert_eq!(w.spin_speed, 0.0);
    }

    #[test]
    fn zero_spin_speed_never_laps() {
        let mut w = Whirl::new(0.0);
        w.spin_up();
        for _ in 0..100 {
            w.tick(1.0);
        }
        assert_eq!(w.revolutions, 0);
        assert!(!w.just_lapped);
        assert_eq!(w.angle, 0.0);
    }

    #[test]
    fn just_lapped_false_when_inactive_tick() {
        let mut w = Whirl::new(TAU);
        w.tick(5.0); // inactive
        assert!(!w.just_lapped);
        assert_eq!(w.revolutions, 0);
    }
}
