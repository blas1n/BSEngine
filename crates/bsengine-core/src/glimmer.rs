use bevy_ecs::prelude::Component;

/// Timed point-source illumination burst: when activated, reveals stealthed
/// entities and attracts AI aggro within `radius` world units for `duration`
/// seconds.
///
/// `activate(duration)` fires the burst (high-watermark: no-op if `duration ≤
/// timer`); sets `just_glimmered` the frame it first fires. `tick(dt)` counts
/// down and sets `just_faded` when the burst expires. Systems query
/// `is_active()` and `in_range(distance)` to determine whether stealth should
/// break or AI should re-evaluate target priority.
///
/// `intensity` [0.0, 1.0] scales the perceived brightness — rendering systems
/// may use it for the light radius multiplier; gameplay systems may threshold
/// it (e.g. only reveal stealth above 0.5).
///
/// Distinct from `Beacon` (persistent static marker with no expiry), `Flare`
/// (area-denial light projectile that travels), and `Flash` (screen-space
/// camera effect): Glimmer is a **timed point-source reveal burst** attached
/// to a gameplay entity, not to the camera or a projectile.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Glimmer {
    pub duration: f32,
    pub timer: f32,
    /// World-unit radius within which the burst reveals stealth and attracts aggro.
    /// Clamped ≥ 0.0.
    pub radius: f32,
    /// Perceived brightness [0.0, 1.0]. Clamped on construction.
    pub intensity: f32,
    pub just_glimmered: bool,
    pub just_faded: bool,
    pub enabled: bool,
}

impl Glimmer {
    pub fn new(radius: f32, intensity: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            radius: radius.max(0.0),
            intensity: intensity.clamp(0.0, 1.0),
            just_glimmered: false,
            just_faded: false,
            enabled: true,
        }
    }

    /// Fire the illumination burst for `duration` seconds. High-watermark:
    /// replaces the current timer only when `duration > timer`. Sets
    /// `just_glimmered` on the first activation (inactive → active). No-op
    /// when disabled or `duration ≤ 0`.
    pub fn activate(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_glimmered = true;
            }
        }
    }

    /// Advance the burst timer; sets `just_faded` when it expires naturally.
    pub fn tick(&mut self, dt: f32) {
        self.just_glimmered = false;
        self.just_faded = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_faded = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Whether an entity at `distance` world units is within the burst radius.
    pub fn in_range(&self, distance: f32) -> bool {
        self.is_active() && distance <= self.radius
    }

    /// Fraction of the burst duration remaining [1.0 = just fired, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Glimmer {
    fn default() -> Self {
        Self::new(10.0, 0.8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activate_starts_burst() {
        let mut g = Glimmer::new(10.0, 0.8);
        g.activate(3.0);
        assert!(g.is_active());
        assert!(g.just_glimmered);
    }

    #[test]
    fn activate_extends_on_longer_duration() {
        let mut g = Glimmer::new(10.0, 0.8);
        g.activate(3.0);
        g.tick(0.016);
        g.activate(8.0);
        assert!((g.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn activate_no_extend_on_shorter_duration() {
        let mut g = Glimmer::new(10.0, 0.8);
        g.activate(8.0);
        g.activate(3.0);
        assert!((g.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn just_glimmered_not_set_on_extend() {
        let mut g = Glimmer::new(10.0, 0.8);
        g.activate(3.0);
        g.tick(0.016);
        g.activate(8.0);
        assert!(!g.just_glimmered);
    }

    #[test]
    fn tick_expires_burst() {
        let mut g = Glimmer::new(10.0, 0.8);
        g.activate(2.0);
        g.tick(2.1);
        assert!(!g.is_active());
        assert!(g.just_faded);
    }

    #[test]
    fn tick_clears_just_glimmered() {
        let mut g = Glimmer::new(10.0, 0.8);
        g.activate(3.0);
        g.tick(0.016);
        assert!(!g.just_glimmered);
    }

    #[test]
    fn in_range_true_within_radius() {
        let mut g = Glimmer::new(10.0, 0.8);
        g.activate(3.0);
        assert!(g.in_range(5.0));
        assert!(g.in_range(10.0));
    }

    #[test]
    fn in_range_false_beyond_radius() {
        let mut g = Glimmer::new(10.0, 0.8);
        g.activate(3.0);
        assert!(!g.in_range(11.0));
    }

    #[test]
    fn in_range_false_when_inactive() {
        let g = Glimmer::new(10.0, 0.8);
        assert!(!g.in_range(5.0));
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut g = Glimmer::new(10.0, 0.8);
        g.activate(4.0);
        g.tick(2.0);
        assert!((g.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_zero_when_inactive() {
        let g = Glimmer::new(10.0, 0.8);
        assert!((g.remaining_fraction()).abs() < 1e-5);
    }

    #[test]
    fn disabled_activate_no_op() {
        let mut g = Glimmer::new(10.0, 0.8);
        g.enabled = false;
        g.activate(3.0);
        assert!(!g.is_active());
    }

    #[test]
    fn activate_zero_duration_no_op() {
        let mut g = Glimmer::new(10.0, 0.8);
        g.activate(0.0);
        assert!(!g.is_active());
    }

    #[test]
    fn intensity_clamped_on_construction() {
        let g = Glimmer::new(10.0, 1.5);
        assert!((g.intensity - 1.0).abs() < 1e-5);
        let g2 = Glimmer::new(10.0, -0.5);
        assert!(g2.intensity.abs() < 1e-5);
    }
}
