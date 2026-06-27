use bevy_ecs::prelude::Component;

/// Seismic vibration source: periodically emits ground-tremor pulses that can
/// stagger or disrupt entities within `radius` world units.
///
/// While active, call `tick(dt)` every frame; it returns `true` on the frame a
/// tremor pulse fires. When `tick` returns `true`, `just_pulsed` is also set.
/// The owning system is responsible for querying nearby entities and applying
/// `stagger_strength` as a disruption impulse.
///
/// Call `activate()` to start tremor generation and `deactivate()` to stop it
/// without resetting the timer. `reset()` stops and resets the timer.
///
/// Distinct from `Explosion` (single burst, immediate radius damage),
/// `Shockwave` (directional linear blast), and `Screen_shake` (camera effect
/// only): Tremor is a **repeating seismic pulse source** — it fires at a
/// regular interval for as long as it is active, affecting ground-level
/// entities with a stagger impulse each time.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Tremor {
    /// Seconds between seismic pulses. Clamped ≥ 0.01 to prevent division-by-zero.
    pub pulse_interval: f32,
    pub pulse_timer: f32,
    /// World-unit radius within which other entities receive the stagger impulse.
    /// Clamped ≥ 0.0.
    pub radius: f32,
    /// Strength of the stagger impulse delivered to entities in range.
    /// Clamped ≥ 0.0.
    pub stagger_strength: f32,
    pub active: bool,
    pub just_pulsed: bool,
    pub enabled: bool,
}

impl Tremor {
    pub fn new(pulse_interval: f32, radius: f32, stagger_strength: f32) -> Self {
        Self {
            pulse_interval: pulse_interval.max(0.01),
            pulse_timer: 0.0,
            radius: radius.max(0.0),
            stagger_strength: stagger_strength.max(0.0),
            active: false,
            just_pulsed: false,
            enabled: true,
        }
    }

    /// Begin emitting tremor pulses. No-op when disabled or already active.
    pub fn activate(&mut self) {
        if self.enabled && !self.active {
            self.active = true;
        }
    }

    /// Stop emitting pulses (timer is preserved).
    pub fn deactivate(&mut self) {
        self.active = false;
    }

    /// Stop and reset the pulse timer.
    pub fn reset(&mut self) {
        self.active = false;
        self.pulse_timer = 0.0;
    }

    /// Advance the tremor timer. Returns `true` on the frame a pulse fires.
    /// Also sets `just_pulsed` that frame. No-op (returns `false`) when not
    /// active or disabled.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.just_pulsed = false;

        if !self.active || !self.enabled {
            return false;
        }

        self.pulse_timer += dt;
        if self.pulse_timer >= self.pulse_interval {
            self.pulse_timer -= self.pulse_interval;
            self.just_pulsed = true;
            return true;
        }

        false
    }

    pub fn is_active(&self) -> bool {
        self.active && self.enabled
    }

    /// Whether an entity at `distance` world units is within the tremor radius.
    pub fn in_range(&self, distance: f32) -> bool {
        self.is_active() && distance <= self.radius
    }

    /// Fraction through the current pulse interval [0.0 → 1.0 → pulse].
    pub fn pulse_fraction(&self) -> f32 {
        if self.pulse_interval <= 0.0 {
            return 0.0;
        }
        (self.pulse_timer / self.pulse_interval).clamp(0.0, 1.0)
    }
}

impl Default for Tremor {
    fn default() -> Self {
        Self::new(0.5, 6.0, 25.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activate_starts_tremor() {
        let mut t = Tremor::new(0.5, 6.0, 25.0);
        t.activate();
        assert!(t.is_active());
    }

    #[test]
    fn deactivate_stops_tremor() {
        let mut t = Tremor::new(0.5, 6.0, 25.0);
        t.activate();
        t.deactivate();
        assert!(!t.is_active());
    }

    #[test]
    fn reset_stops_and_clears_timer() {
        let mut t = Tremor::new(0.5, 6.0, 25.0);
        t.activate();
        t.tick(0.3);
        t.reset();
        assert!(!t.active);
        assert!(t.pulse_timer < 1e-5);
    }

    #[test]
    fn tick_returns_false_before_interval() {
        let mut t = Tremor::new(0.5, 6.0, 25.0);
        t.activate();
        assert!(!t.tick(0.3));
        assert!(!t.just_pulsed);
    }

    #[test]
    fn tick_returns_true_at_interval() {
        let mut t = Tremor::new(0.5, 6.0, 25.0);
        t.activate();
        assert!(t.tick(0.5));
        assert!(t.just_pulsed);
    }

    #[test]
    fn tick_accumulates_partial_steps() {
        let mut t = Tremor::new(0.5, 6.0, 25.0);
        t.activate();
        t.tick(0.3);
        assert!(t.tick(0.25)); // 0.3 + 0.25 = 0.55 ≥ 0.5
        assert!(t.just_pulsed);
    }

    #[test]
    fn tick_clears_just_pulsed_next_frame() {
        let mut t = Tremor::new(0.5, 6.0, 25.0);
        t.activate();
        t.tick(0.5);
        assert!(t.just_pulsed);
        t.tick(0.016);
        assert!(!t.just_pulsed);
    }

    #[test]
    fn tick_no_op_when_inactive() {
        let mut t = Tremor::new(0.5, 6.0, 25.0);
        assert!(!t.tick(1.0));
        assert!(!t.just_pulsed);
        assert!(t.pulse_timer < 1e-5);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut t = Tremor::new(0.5, 6.0, 25.0);
        t.activate();
        t.enabled = false;
        assert!(!t.tick(1.0));
    }

    #[test]
    fn in_range_true_within_radius() {
        let mut t = Tremor::new(0.5, 6.0, 25.0);
        t.activate();
        assert!(t.in_range(4.0));
        assert!(t.in_range(6.0));
    }

    #[test]
    fn in_range_false_beyond_radius() {
        let mut t = Tremor::new(0.5, 6.0, 25.0);
        t.activate();
        assert!(!t.in_range(7.0));
    }

    #[test]
    fn in_range_false_when_inactive() {
        let t = Tremor::new(0.5, 6.0, 25.0);
        assert!(!t.in_range(1.0));
    }

    #[test]
    fn pulse_fraction_advances() {
        let mut t = Tremor::new(0.5, 6.0, 25.0);
        t.activate();
        t.tick(0.25);
        assert!((t.pulse_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn pulse_interval_clamped() {
        let t = Tremor::new(-1.0, 6.0, 25.0); // < 0.01 → 0.01
        assert!(t.pulse_interval >= 0.01);
    }

    #[test]
    fn disabled_activate_no_op() {
        let mut t = Tremor::new(0.5, 6.0, 25.0);
        t.enabled = false;
        t.activate();
        assert!(!t.active);
    }
}
