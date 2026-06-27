use bevy_ecs::prelude::Component;

/// A persistent spectral companion that orbits the entity. While active the
/// wisp completes one full orbit every `orbit_period` seconds; on each orbit
/// completion it fires `just_pulsed` and increments `pulse_count`, allowing
/// systems to apply healing, damage auras, or other per-pulse effects driven
/// by `heal_per_pulse`.
///
/// `activate()` starts the wisp orbiting. No-op when already active or
/// disabled.
///
/// `deactivate()` stops the wisp. Resets `orbit_timer` to 0.0. No-op when
/// not active.
///
/// `tick(dt)` clears `just_pulsed` at start; when active: advances
/// `orbit_timer`; on completing a full orbit (`orbit_timer >= orbit_period`)
/// wraps the timer, fires `just_pulsed`, and increments `pulse_count`. No-op
/// when disabled.
///
/// `is_active()` returns `active && enabled`.
///
/// `orbit_fraction()` returns `(orbit_timer / orbit_period).clamp(0.0, 1.0)` —
/// how far through the current orbit the wisp is.
///
/// Distinct from `Aura` (passive always-on area effect with no pulse rhythm),
/// `Homing` (projectile tracking behaviour), `Regen` (flat health recovery per
/// second with no orbital metaphor), and `Beacon` (stationary broadcast): Wisp
/// is a **companion orbital** — it pulses on a fixed rhythm tied to a
/// conceptual orbit, letting systems hook into each completion to apply
/// per-pulse effects rather than continuous scaling.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wisp {
    /// Seconds elapsed in the current orbit.
    pub orbit_timer: f32,
    /// Seconds to complete one full orbit. Clamped >= 0.1.
    pub orbit_period: f32,
    /// Healing (or other scalar value) applied by consuming systems per pulse.
    /// Clamped >= 0.0.
    pub heal_per_pulse: f32,
    /// Total number of orbit completions since the wisp was activated.
    pub pulse_count: u32,
    pub active: bool,
    pub just_pulsed: bool,
    pub enabled: bool,
}

impl Wisp {
    pub fn new(orbit_period: f32, heal_per_pulse: f32) -> Self {
        Self {
            orbit_timer: 0.0,
            orbit_period: orbit_period.max(0.1),
            heal_per_pulse: heal_per_pulse.max(0.0),
            pulse_count: 0,
            active: false,
            just_pulsed: false,
            enabled: true,
        }
    }

    /// Start the wisp orbiting. No-op when already active or disabled.
    pub fn activate(&mut self) {
        if !self.enabled || self.active {
            return;
        }
        self.active = true;
    }

    /// Stop the wisp and reset the orbit timer. No-op when not active.
    pub fn deactivate(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        self.orbit_timer = 0.0;
    }

    /// Advance the orbit. Clears `just_pulsed` first; when active, increments
    /// `orbit_timer`; fires `just_pulsed` and increments `pulse_count` on
    /// each full orbit completion (timer wraps). No-op when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_pulsed = false;

        if !self.enabled || !self.active {
            return;
        }

        self.orbit_timer += dt;
        if self.orbit_timer >= self.orbit_period {
            self.orbit_timer -= self.orbit_period;
            self.just_pulsed = true;
            self.pulse_count += 1;
        }
    }

    /// `true` when the wisp is actively orbiting and the component is enabled.
    pub fn is_active(&self) -> bool {
        self.active && self.enabled
    }

    /// Fraction of the current orbit completed [0.0, 1.0].
    pub fn orbit_fraction(&self) -> f32 {
        (self.orbit_timer / self.orbit_period).clamp(0.0, 1.0)
    }
}

impl Default for Wisp {
    fn default() -> Self {
        Self::new(3.0, 10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_inactive() {
        let w = Wisp::new(3.0, 10.0);
        assert!(!w.active);
        assert!(!w.is_active());
        assert_eq!(w.orbit_timer, 0.0);
        assert_eq!(w.pulse_count, 0);
    }

    #[test]
    fn activate_sets_active() {
        let mut w = Wisp::new(3.0, 10.0);
        w.activate();
        assert!(w.active);
        assert!(w.is_active());
    }

    #[test]
    fn activate_no_op_when_already_active() {
        let mut w = Wisp::new(3.0, 10.0);
        w.activate();
        w.tick(1.0); // advance timer
        w.activate(); // should not reset timer
        assert!((w.orbit_timer - 1.0).abs() < 1e-5);
    }

    #[test]
    fn activate_no_op_when_disabled() {
        let mut w = Wisp::new(3.0, 10.0);
        w.enabled = false;
        w.activate();
        assert!(!w.active);
    }

    #[test]
    fn deactivate_clears_active() {
        let mut w = Wisp::new(3.0, 10.0);
        w.activate();
        w.deactivate();
        assert!(!w.active);
    }

    #[test]
    fn deactivate_resets_orbit_timer() {
        let mut w = Wisp::new(3.0, 10.0);
        w.activate();
        w.tick(1.5);
        w.deactivate();
        assert_eq!(w.orbit_timer, 0.0);
    }

    #[test]
    fn deactivate_no_op_when_already_inactive() {
        let mut w = Wisp::new(3.0, 10.0);
        w.deactivate(); // should not panic
        assert!(!w.active);
    }

    #[test]
    fn tick_advances_orbit_timer_while_active() {
        let mut w = Wisp::new(3.0, 10.0);
        w.activate();
        w.tick(1.0);
        assert!((w.orbit_timer - 1.0).abs() < 1e-5);
    }

    #[test]
    fn tick_no_advance_when_inactive() {
        let mut w = Wisp::new(3.0, 10.0);
        w.tick(1.0);
        assert_eq!(w.orbit_timer, 0.0);
    }

    #[test]
    fn tick_no_advance_when_disabled() {
        let mut w = Wisp::new(3.0, 10.0);
        w.activate();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.orbit_timer, 0.0);
    }

    #[test]
    fn tick_fires_just_pulsed_on_full_orbit() {
        let mut w = Wisp::new(2.0, 10.0);
        w.activate();
        w.tick(2.0);
        assert!(w.just_pulsed);
    }

    #[test]
    fn tick_increments_pulse_count_on_full_orbit() {
        let mut w = Wisp::new(2.0, 10.0);
        w.activate();
        w.tick(2.0);
        assert_eq!(w.pulse_count, 1);
    }

    #[test]
    fn tick_wraps_orbit_timer_after_pulse() {
        let mut w = Wisp::new(2.0, 10.0);
        w.activate();
        w.tick(2.5); // 0.5 leftover
        assert!((w.orbit_timer - 0.5).abs() < 1e-5);
    }

    #[test]
    fn tick_no_just_pulsed_before_orbit_completes() {
        let mut w = Wisp::new(3.0, 10.0);
        w.activate();
        w.tick(1.0);
        assert!(!w.just_pulsed);
    }

    #[test]
    fn tick_clears_just_pulsed_next_frame() {
        let mut w = Wisp::new(1.0, 10.0);
        w.activate();
        w.tick(1.0); // just_pulsed = true
        w.tick(0.016); // cleared
        assert!(!w.just_pulsed);
    }

    #[test]
    fn tick_multiple_orbits_accumulate_pulse_count() {
        let mut w = Wisp::new(1.0, 10.0);
        w.activate();
        for _ in 0..5 {
            w.tick(1.0);
        }
        assert_eq!(w.pulse_count, 5);
    }

    #[test]
    fn tick_only_one_pulse_per_frame_even_with_large_dt() {
        let mut w = Wisp::new(1.0, 10.0);
        w.activate();
        w.tick(3.0); // 3 orbits worth of dt
        assert!(w.just_pulsed); // only one just_pulsed per tick call
        assert_eq!(w.pulse_count, 1);
    }

    #[test]
    fn is_active_false_when_disabled() {
        let mut w = Wisp::new(3.0, 10.0);
        w.activate();
        w.enabled = false;
        assert!(!w.is_active());
    }

    #[test]
    fn is_active_false_when_not_activated() {
        let w = Wisp::new(3.0, 10.0);
        assert!(!w.is_active());
    }

    #[test]
    fn orbit_fraction_zero_when_no_progress() {
        let w = Wisp::new(3.0, 10.0);
        assert_eq!(w.orbit_fraction(), 0.0);
    }

    #[test]
    fn orbit_fraction_half_at_midpoint() {
        let mut w = Wisp::new(4.0, 10.0);
        w.activate();
        w.tick(2.0); // half of 4s period
        assert!((w.orbit_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn orbit_fraction_resets_after_pulse() {
        let mut w = Wisp::new(2.0, 10.0);
        w.activate();
        w.tick(2.5); // 0.5s into next orbit after 2s period
                     // fraction = 0.5 / 2.0 = 0.25
        assert!((w.orbit_fraction() - 0.25).abs() < 1e-5);
    }

    #[test]
    fn orbit_period_clamped_to_min() {
        let w = Wisp::new(0.0, 10.0);
        assert!((w.orbit_period - 0.1).abs() < 1e-5);
    }

    #[test]
    fn heal_per_pulse_clamped_to_zero() {
        let w = Wisp::new(3.0, -5.0);
        assert_eq!(w.heal_per_pulse, 0.0);
    }

    #[test]
    fn pulse_count_persists_across_deactivate_reactivate() {
        let mut w = Wisp::new(1.0, 10.0);
        w.activate();
        w.tick(1.0);
        assert_eq!(w.pulse_count, 1);
        w.deactivate();
        w.activate();
        w.tick(1.0);
        assert_eq!(w.pulse_count, 2);
    }

    #[test]
    fn orbit_timer_resets_on_deactivate_not_on_just_pulsed() {
        let mut w = Wisp::new(2.0, 10.0);
        w.activate();
        w.tick(2.5); // pulsed, 0.5 leftover
        assert!((w.orbit_timer - 0.5).abs() < 1e-5); // not reset — still orbiting
        w.deactivate();
        assert_eq!(w.orbit_timer, 0.0); // now reset
    }

    #[test]
    fn just_pulsed_false_when_tick_inactive() {
        let mut w = Wisp::new(1.0, 10.0);
        w.tick(5.0); // inactive, should not pulse
        assert!(!w.just_pulsed);
        assert_eq!(w.pulse_count, 0);
    }

    #[test]
    fn reactivation_after_deactivate_starts_fresh_orbit() {
        let mut w = Wisp::new(2.0, 10.0);
        w.activate();
        w.tick(1.0);
        w.deactivate(); // timer reset to 0
        w.activate();
        w.tick(1.0); // 1s into fresh orbit, no pulse yet
        assert!(!w.just_pulsed);
        assert_eq!(w.pulse_count, 0);
    }
}
