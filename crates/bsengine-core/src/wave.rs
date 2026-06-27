use bevy_ecs::prelude::Component;

/// Expanding shockwave that propagates outward from an origin point, dealing
/// damage to any entity whose distance from the origin falls within the wave
/// front at the time of contact.
///
/// Call `emit()` to launch the wave. Each frame, advance with `tick(dt)`:
/// `radius` grows by `expansion_rate * dt`. When `radius` reaches `max_radius`
/// the wave dissipates and `just_dissipated` is set.
///
/// Hit detection: call `has_reached(distance)` to check whether the wave front
/// has swept past a given distance from the origin this frame. Systems should
/// combine this with a "not yet hit" marker to avoid double-damage.
///
/// Distinct from `Explosion` (instant full-radius burst) and `Pulse` (repeated
/// periodic ping): Wave is a single expanding ring that deals damage as it
/// sweeps outward, not all at once.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wave {
    /// Current radius of the wave front (world units).
    pub radius: f32,
    /// Maximum radius before the wave dissipates.
    pub max_radius: f32,
    /// Expansion speed (world units per second).
    pub expansion_rate: f32,
    /// Damage dealt to each entity the wave front passes through.
    pub damage: f32,
    pub just_emitted: bool,
    pub just_dissipated: bool,
    pub enabled: bool,
}

impl Wave {
    pub fn new(max_radius: f32, expansion_rate: f32, damage: f32) -> Self {
        Self {
            radius: 0.0,
            max_radius: max_radius.max(0.0),
            expansion_rate: expansion_rate.max(0.0),
            damage: damage.max(0.0),
            just_emitted: false,
            just_dissipated: false,
            enabled: true,
        }
    }

    /// Launch the wave from radius 0. No-op when disabled or already active.
    pub fn emit(&mut self) {
        if !self.enabled || self.is_active() {
            return;
        }
        self.radius = 0.0;
        self.just_emitted = true;
    }

    /// Advance the wave front by `dt` seconds. Returns `true` on the frame the
    /// wave dissipates.
    pub fn tick(&mut self, dt: f32) -> bool {
        let was_just_emitted = self.just_emitted;
        self.just_emitted = false;
        self.just_dissipated = false;

        if was_just_emitted || (self.radius > 0.0 && self.radius < self.max_radius) {
            self.radius += self.expansion_rate * dt;
            if self.radius >= self.max_radius {
                self.radius = self.max_radius;
                self.just_dissipated = true;
                return true;
            }
        }
        false
    }

    /// True while the wave is expanding (emitted but not yet dissipated).
    pub fn is_active(&self) -> bool {
        self.just_emitted || (self.radius > 0.0 && self.radius < self.max_radius)
    }

    /// True if the wave front has swept past `distance` world units from the
    /// origin. Use this each frame to check whether an entity at that distance
    /// should be hit.
    pub fn has_reached(&self, distance: f32) -> bool {
        self.radius >= distance
    }

    /// Fraction of the wave's travel completed [0.0 = just emitted, 1.0 = dissipated].
    pub fn progress_fraction(&self) -> f32 {
        if self.max_radius <= 0.0 {
            return 1.0;
        }
        (self.radius / self.max_radius).clamp(0.0, 1.0)
    }
}

impl Default for Wave {
    fn default() -> Self {
        Self::new(10.0, 8.0, 25.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_activates_wave() {
        let mut w = Wave::new(10.0, 5.0, 20.0);
        w.emit();
        assert!(w.is_active());
        assert!(w.just_emitted);
    }

    #[test]
    fn emit_no_op_when_already_active() {
        let mut w = Wave::new(10.0, 5.0, 20.0);
        w.emit();
        w.tick(0.5);
        let r_before = w.radius;
        w.emit(); // should not reset
        assert!((w.radius - r_before).abs() < 1e-5);
    }

    #[test]
    fn tick_expands_radius() {
        let mut w = Wave::new(10.0, 4.0, 20.0);
        w.emit();
        w.tick(1.0);
        assert!((w.radius - 4.0).abs() < 1e-3);
    }

    #[test]
    fn tick_dissipates_at_max_radius() {
        let mut w = Wave::new(5.0, 10.0, 20.0);
        w.emit();
        let done = w.tick(1.0); // 10 * 1.0 = 10 > max 5
        assert!(done);
        assert!(w.just_dissipated);
        assert!(!w.is_active());
        assert!((w.radius - 5.0).abs() < 1e-5);
    }

    #[test]
    fn has_reached_true_when_radius_past_distance() {
        let mut w = Wave::new(10.0, 5.0, 20.0);
        w.emit();
        w.tick(2.0); // radius = 10
        assert!(w.has_reached(9.0));
        assert!(w.has_reached(10.0));
    }

    #[test]
    fn has_reached_false_when_not_expanded_yet() {
        let mut w = Wave::new(10.0, 5.0, 20.0);
        w.emit();
        w.tick(0.5); // radius = 2.5
        assert!(!w.has_reached(5.0));
    }

    #[test]
    fn progress_fraction_at_half() {
        let mut w = Wave::new(10.0, 5.0, 20.0);
        w.emit();
        w.tick(1.0); // radius = 5
        assert!((w.progress_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_emit_no_op() {
        let mut w = Wave::new(10.0, 5.0, 20.0);
        w.enabled = false;
        w.emit();
        assert!(!w.is_active());
    }

    #[test]
    fn tick_clears_just_emitted() {
        let mut w = Wave::new(10.0, 5.0, 20.0);
        w.emit();
        w.tick(0.016);
        assert!(!w.just_emitted);
    }

    #[test]
    fn not_active_before_emit() {
        let w = Wave::new(10.0, 5.0, 20.0);
        assert!(!w.is_active());
    }
}
