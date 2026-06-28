use bevy_ecs::prelude::Component;

/// Peak-elevation accumulation tracker named after vertex, the noun
/// that mathematical and anatomical tradition have pressed into
/// service for the same fundamental concept: the highest point, the
/// point of greatest extremity, the point that defines a shape by
/// its farthest extension from some assumed baseline. The word comes
/// from the Latin vertex, meaning whirlpool, eddy, crown of the
/// head, and highest point — from vertere, to turn — because the
/// crown of the head was understood as the point around which the
/// vortex of hair naturally organises itself, and from there the
/// term generalised upward through anatomy into mathematics. In
/// geometry, a vertex is a point where two or more edges of a
/// polygon meet: the corners of a triangle are its vertices, the
/// tip of a cone is its vertex, and the entire discipline of graph
/// theory uses vertex as its term for a node — a point from which
/// edges extend outward. In computer graphics, vertices are the
/// fundamental data unit from which meshes are built: every visible
/// surface in a rendered world is decomposed into triangles whose
/// corners are vertices, each carrying position, normal, UV
/// coordinates, and whatever other per-point data the shading system
/// requires. The vertex shader is the first programmable stage of
/// the modern graphics pipeline, executing once per vertex before
/// the rasterizer interpolates between them to fill the interior of
/// each triangle. In calculus, the vertex of a parabola is the
/// point at which the function reaches its minimum or maximum, the
/// point of inflection where the curve reverses its direction of
/// travel. In anatomy, the vertex is the crown of the skull — the
/// highest point of the head when the subject stands erect and gazes
/// at the horizon. All of these senses converge on the same abstract
/// concept: the extremum, the point beyond which the structure
/// cannot extend without changing its fundamental nature. `apex`
/// builds via `ascend(amount)` and accumulates passively at
/// `ascent_rate` per second in `tick(dt)` or descends via
/// `descend(amount)`.
///
/// Models peak-elevation fill levels, mesh-vertex saturation
/// bars, trajectory-apex accumulators, polygon-corner gauges,
/// shader-load fill levels, graph-node saturation indicators,
/// parabola-peak accumulation bars, cranial-pressure meters,
/// summit-approach fill levels, or any mechanic where a projectile,
/// creature, or system climbs toward its highest point — ascending
/// arc by ascending arc — until the apex is reached and the
/// inevitable descent begins, or until something arrests the climb
/// short of the maximum and the peak is never attained.
///
/// `ascend(amount)` adds apex; fires `just_peaked` when first
/// reaching `max_apex`. No-op when disabled.
///
/// `descend(amount)` reduces apex immediately; fires `just_grounded`
/// when reaching 0. No-op when disabled or already grounded.
///
/// `tick(dt)` clears both flags, then increases apex by
/// `ascent_rate * dt` (capped at `max_apex`). Fires `just_peaked`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_peaked()` returns `apex >= max_apex && enabled`.
///
/// `is_grounded()` returns `apex == 0.0` (not gated by `enabled`).
///
/// `apex_fraction()` returns `(apex / max_apex).clamp(0, 1)`.
///
/// `effective_elevation(scale)` returns `scale * apex_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — ascends at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Vertex {
    pub apex: f32,
    pub max_apex: f32,
    pub ascent_rate: f32,
    pub just_peaked: bool,
    pub just_grounded: bool,
    pub enabled: bool,
}

impl Vertex {
    pub fn new(max_apex: f32, ascent_rate: f32) -> Self {
        Self {
            apex: 0.0,
            max_apex: max_apex.max(0.1),
            ascent_rate: ascent_rate.max(0.0),
            just_peaked: false,
            just_grounded: false,
            enabled: true,
        }
    }

    /// Add apex; fires `just_peaked` when first reaching max.
    /// No-op when disabled.
    pub fn ascend(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.apex < self.max_apex;
        self.apex = (self.apex + amount).min(self.max_apex);
        if was_below && self.apex >= self.max_apex {
            self.just_peaked = true;
        }
    }

    /// Reduce apex; fires `just_grounded` when reaching 0.
    /// No-op when disabled or already grounded.
    pub fn descend(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.apex <= 0.0 {
            return;
        }
        self.apex = (self.apex - amount).max(0.0);
        if self.apex <= 0.0 {
            self.just_grounded = true;
        }
    }

    /// Clear flags, then increase apex by `ascent_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_peaked = false;
        self.just_grounded = false;
        if self.enabled && self.ascent_rate > 0.0 && self.apex < self.max_apex {
            let was_below = self.apex < self.max_apex;
            self.apex = (self.apex + self.ascent_rate * dt).min(self.max_apex);
            if was_below && self.apex >= self.max_apex {
                self.just_peaked = true;
            }
        }
    }

    /// `true` when apex is at maximum and component is enabled.
    pub fn is_peaked(&self) -> bool {
        self.apex >= self.max_apex && self.enabled
    }

    /// `true` when apex is 0 (not gated by `enabled`).
    pub fn is_grounded(&self) -> bool {
        self.apex == 0.0
    }

    /// Fraction of maximum apex [0.0, 1.0].
    pub fn apex_fraction(&self) -> f32 {
        (self.apex / self.max_apex).clamp(0.0, 1.0)
    }

    /// Returns `scale * apex_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_elevation(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.apex_fraction()
    }
}

impl Default for Vertex {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v() -> Vertex {
        Vertex::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_grounded() {
        let v = v();
        assert_eq!(v.apex, 0.0);
        assert!(v.is_grounded());
        assert!(!v.is_peaked());
    }

    #[test]
    fn new_clamps_max_apex() {
        let v = Vertex::new(-5.0, 1.5);
        assert!((v.max_apex - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_ascent_rate() {
        let v = Vertex::new(100.0, -1.5);
        assert_eq!(v.ascent_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let v = Vertex::default();
        assert!((v.max_apex - 100.0).abs() < 1e-5);
        assert!((v.ascent_rate - 1.5).abs() < 1e-5);
    }

    // --- ascend ---

    #[test]
    fn ascend_adds_apex() {
        let mut v = v();
        v.ascend(40.0);
        assert!((v.apex - 40.0).abs() < 1e-3);
    }

    #[test]
    fn ascend_clamps_at_max() {
        let mut v = v();
        v.ascend(200.0);
        assert!((v.apex - 100.0).abs() < 1e-3);
    }

    #[test]
    fn ascend_fires_just_peaked_at_max() {
        let mut v = v();
        v.ascend(100.0);
        assert!(v.just_peaked);
        assert!(v.is_peaked());
    }

    #[test]
    fn ascend_no_just_peaked_when_already_at_max() {
        let mut v = v();
        v.apex = 100.0;
        v.ascend(10.0);
        assert!(!v.just_peaked);
    }

    #[test]
    fn ascend_no_op_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.ascend(50.0);
        assert_eq!(v.apex, 0.0);
    }

    #[test]
    fn ascend_no_op_when_amount_zero() {
        let mut v = v();
        v.ascend(0.0);
        assert_eq!(v.apex, 0.0);
    }

    // --- descend ---

    #[test]
    fn descend_reduces_apex() {
        let mut v = v();
        v.apex = 60.0;
        v.descend(20.0);
        assert!((v.apex - 40.0).abs() < 1e-3);
    }

    #[test]
    fn descend_clamps_at_zero() {
        let mut v = v();
        v.apex = 30.0;
        v.descend(200.0);
        assert_eq!(v.apex, 0.0);
    }

    #[test]
    fn descend_fires_just_grounded_at_zero() {
        let mut v = v();
        v.apex = 30.0;
        v.descend(30.0);
        assert!(v.just_grounded);
    }

    #[test]
    fn descend_no_op_when_already_grounded() {
        let mut v = v();
        v.descend(10.0);
        assert!(!v.just_grounded);
    }

    #[test]
    fn descend_no_op_when_disabled() {
        let mut v = v();
        v.apex = 50.0;
        v.enabled = false;
        v.descend(50.0);
        assert!((v.apex - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_apex() {
        let mut v = v(); // rate=1.5
        v.tick(4.0); // 0 + 1.5*4 = 6
        assert!((v.apex - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_peaked_on_apex_to_max() {
        let mut v = Vertex::new(100.0, 200.0);
        v.apex = 95.0;
        v.tick(1.0);
        assert!(v.just_peaked);
        assert!(v.is_peaked());
    }

    #[test]
    fn tick_no_build_when_already_peaked() {
        let mut v = v();
        v.apex = 100.0;
        v.tick(1.0);
        assert!(!v.just_peaked);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut v = Vertex::new(100.0, 0.0);
        v.tick(100.0);
        assert_eq!(v.apex, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.tick(1.0);
        assert_eq!(v.apex, 0.0);
    }

    #[test]
    fn tick_clears_just_peaked() {
        let mut v = Vertex::new(100.0, 200.0);
        v.apex = 95.0;
        v.tick(1.0);
        v.tick(0.016);
        assert!(!v.just_peaked);
    }

    #[test]
    fn tick_clears_just_grounded() {
        let mut v = v();
        v.apex = 10.0;
        v.descend(10.0);
        v.tick(0.016);
        assert!(!v.just_grounded);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut v = v(); // rate=1.5
        v.tick(6.0); // 1.5*6 = 9
        assert!((v.apex - 9.0).abs() < 1e-3);
    }

    // --- is_peaked / is_grounded ---

    #[test]
    fn is_peaked_false_when_disabled() {
        let mut v = v();
        v.apex = 100.0;
        v.enabled = false;
        assert!(!v.is_peaked());
    }

    #[test]
    fn is_grounded_not_gated_by_enabled() {
        let mut v = v();
        v.enabled = false;
        assert!(v.is_grounded());
    }

    // --- apex_fraction / effective_elevation ---

    #[test]
    fn apex_fraction_zero_when_grounded() {
        assert_eq!(v().apex_fraction(), 0.0);
    }

    #[test]
    fn apex_fraction_half_at_midpoint() {
        let mut v = v();
        v.apex = 50.0;
        assert!((v.apex_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_elevation_zero_when_grounded() {
        assert_eq!(v().effective_elevation(100.0), 0.0);
    }

    #[test]
    fn effective_elevation_scales_with_apex() {
        let mut v = v();
        v.apex = 75.0;
        assert!((v.effective_elevation(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_elevation_zero_when_disabled() {
        let mut v = v();
        v.apex = 50.0;
        v.enabled = false;
        assert_eq!(v.effective_elevation(100.0), 0.0);
    }
}
