use bevy_ecs::prelude::Component;

/// Path-traversal accumulation tracker named after way, the noun
/// and adverb meaning a road, path, or track affording passage
/// from one place to another; the course taken in going from one
/// place to another; the distance to be traversed — from the Old
/// English weg (road, path, course), from the Proto-Germanic
/// wegaz, from the Proto-Indo-European root wegh- (to go, move,
/// transport in a vehicle). The root wegh- spread across the Indo-
/// European world to produce Latin via (way, road), Sanskrit vaha-
/// (carrying, bearing), and the Germanic vehicle, way, and wagon —
/// all connected by the idea of movement along a defined course.
/// English way contains multiple senses in a single short word:
/// the physical road (the way to town), the method (the way to
/// solve it), the direction (which way did they go?), the distance
/// (a long way away), and the informal intensifier (way too much)
/// — all derivatives of the core idea of a passage, route, or
/// manner of proceeding. In theology and philosophy, the Way
/// (Tao in Chinese, Hodos in Greek, Dharma in Sanskrit) is the
/// fundamental principle or path that underlies right action and
/// cosmic order — the supreme way that transcends all individual
/// ways. In navigation, the way is a ship's progress through the
/// water; to have way is to be moving. In game mechanics, a way
/// mechanic models the accumulation of traversal progress — the
/// slow build of path completion, route discovery, or journey
/// advancement that eventually reaches the destination. `progress`
/// builds via `advance(amount)` and accumulates passively at
/// `traverse_rate` per second in `tick(dt)` or retreats via
/// `retreat(amount)`.
///
/// Models path-traversal fill levels, route-completion saturation
/// bars, journey-progress accumulators, navigation-advance gauges,
/// waypoint-approach fill levels, discovery-saturation indicators,
/// exploration-completion accumulation bars, travel-progress
/// meters, passage-completion fill levels, or any mechanic where
/// a character, vehicle, or expedition slowly accumulates traversal
/// progress along a defined route — the path unfolds step by step,
/// the destination draws incrementally closer, and the journey
/// eventually reaches its end when the accumulated progress equals
/// the total distance to be covered.
///
/// `advance(amount)` adds progress; fires `just_arrived` when
/// first reaching `max_progress`. No-op when disabled.
///
/// `retreat(amount)` reduces progress immediately; fires
/// `just_turned` when reaching 0. No-op when disabled or already
/// at start.
///
/// `tick(dt)` clears both flags, then increases progress by
/// `traverse_rate * dt` (capped at `max_progress`). Fires
/// `just_arrived` when first reaching max. No-op when disabled or
/// rate is 0.
///
/// `is_arrived()` returns `progress >= max_progress && enabled`.
///
/// `is_at_start()` returns `progress == 0.0` (not gated by
/// `enabled`).
///
/// `progress_fraction()` returns
/// `(progress / max_progress).clamp(0, 1)`.
///
/// `effective_distance(scale)` returns `scale * progress_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — traverses at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Way {
    pub progress: f32,
    pub max_progress: f32,
    pub traverse_rate: f32,
    pub just_arrived: bool,
    pub just_turned: bool,
    pub enabled: bool,
}

impl Way {
    pub fn new(max_progress: f32, traverse_rate: f32) -> Self {
        Self {
            progress: 0.0,
            max_progress: max_progress.max(0.1),
            traverse_rate: traverse_rate.max(0.0),
            just_arrived: false,
            just_turned: false,
            enabled: true,
        }
    }

    /// Add progress; fires `just_arrived` when first reaching max.
    /// No-op when disabled.
    pub fn advance(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.progress < self.max_progress;
        self.progress = (self.progress + amount).min(self.max_progress);
        if was_below && self.progress >= self.max_progress {
            self.just_arrived = true;
        }
    }

    /// Reduce progress; fires `just_turned` when reaching 0.
    /// No-op when disabled or already at start.
    pub fn retreat(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.progress <= 0.0 {
            return;
        }
        self.progress = (self.progress - amount).max(0.0);
        if self.progress <= 0.0 {
            self.just_turned = true;
        }
    }

    /// Clear flags, then increase progress by `traverse_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_arrived = false;
        self.just_turned = false;
        if self.enabled && self.traverse_rate > 0.0 && self.progress < self.max_progress {
            let was_below = self.progress < self.max_progress;
            self.progress = (self.progress + self.traverse_rate * dt).min(self.max_progress);
            if was_below && self.progress >= self.max_progress {
                self.just_arrived = true;
            }
        }
    }

    /// `true` when progress is at maximum and component is enabled.
    pub fn is_arrived(&self) -> bool {
        self.progress >= self.max_progress && self.enabled
    }

    /// `true` when progress is 0 (not gated by `enabled`).
    pub fn is_at_start(&self) -> bool {
        self.progress == 0.0
    }

    /// Fraction of maximum progress [0.0, 1.0].
    pub fn progress_fraction(&self) -> f32 {
        (self.progress / self.max_progress).clamp(0.0, 1.0)
    }

    /// Returns `scale * progress_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_distance(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.progress_fraction()
    }
}

impl Default for Way {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Way {
        Way::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_at_start() {
        let w = w();
        assert_eq!(w.progress, 0.0);
        assert!(w.is_at_start());
        assert!(!w.is_arrived());
    }

    #[test]
    fn new_clamps_max_progress() {
        let w = Way::new(-5.0, 1.5);
        assert!((w.max_progress - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_traverse_rate() {
        let w = Way::new(100.0, -1.5);
        assert_eq!(w.traverse_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Way::default();
        assert!((w.max_progress - 100.0).abs() < 1e-5);
        assert!((w.traverse_rate - 1.5).abs() < 1e-5);
    }

    // --- advance ---

    #[test]
    fn advance_adds_progress() {
        let mut w = w();
        w.advance(40.0);
        assert!((w.progress - 40.0).abs() < 1e-3);
    }

    #[test]
    fn advance_clamps_at_max() {
        let mut w = w();
        w.advance(200.0);
        assert!((w.progress - 100.0).abs() < 1e-3);
    }

    #[test]
    fn advance_fires_just_arrived_at_max() {
        let mut w = w();
        w.advance(100.0);
        assert!(w.just_arrived);
        assert!(w.is_arrived());
    }

    #[test]
    fn advance_no_just_arrived_when_already_at_max() {
        let mut w = w();
        w.progress = 100.0;
        w.advance(10.0);
        assert!(!w.just_arrived);
    }

    #[test]
    fn advance_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.advance(50.0);
        assert_eq!(w.progress, 0.0);
    }

    #[test]
    fn advance_no_op_when_amount_zero() {
        let mut w = w();
        w.advance(0.0);
        assert_eq!(w.progress, 0.0);
    }

    // --- retreat ---

    #[test]
    fn retreat_reduces_progress() {
        let mut w = w();
        w.progress = 60.0;
        w.retreat(20.0);
        assert!((w.progress - 40.0).abs() < 1e-3);
    }

    #[test]
    fn retreat_clamps_at_zero() {
        let mut w = w();
        w.progress = 30.0;
        w.retreat(200.0);
        assert_eq!(w.progress, 0.0);
    }

    #[test]
    fn retreat_fires_just_turned_at_zero() {
        let mut w = w();
        w.progress = 30.0;
        w.retreat(30.0);
        assert!(w.just_turned);
    }

    #[test]
    fn retreat_no_op_when_already_at_start() {
        let mut w = w();
        w.retreat(10.0);
        assert!(!w.just_turned);
    }

    #[test]
    fn retreat_no_op_when_disabled() {
        let mut w = w();
        w.progress = 50.0;
        w.enabled = false;
        w.retreat(50.0);
        assert!((w.progress - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_progress() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.progress - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_arrived_on_progress_to_max() {
        let mut w = Way::new(100.0, 200.0);
        w.progress = 95.0;
        w.tick(1.0);
        assert!(w.just_arrived);
        assert!(w.is_arrived());
    }

    #[test]
    fn tick_no_build_when_already_arrived() {
        let mut w = w();
        w.progress = 100.0;
        w.tick(1.0);
        assert!(!w.just_arrived);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Way::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.progress, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.progress, 0.0);
    }

    #[test]
    fn tick_clears_just_arrived() {
        let mut w = Way::new(100.0, 200.0);
        w.progress = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_arrived);
    }

    #[test]
    fn tick_clears_just_turned() {
        let mut w = w();
        w.progress = 10.0;
        w.retreat(10.0);
        w.tick(0.016);
        assert!(!w.just_turned);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.progress - 9.0).abs() < 1e-3);
    }

    // --- is_arrived / is_at_start ---

    #[test]
    fn is_arrived_false_when_disabled() {
        let mut w = w();
        w.progress = 100.0;
        w.enabled = false;
        assert!(!w.is_arrived());
    }

    #[test]
    fn is_at_start_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_at_start());
    }

    // --- progress_fraction / effective_distance ---

    #[test]
    fn progress_fraction_zero_when_at_start() {
        assert_eq!(w().progress_fraction(), 0.0);
    }

    #[test]
    fn progress_fraction_half_at_midpoint() {
        let mut w = w();
        w.progress = 50.0;
        assert!((w.progress_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_distance_zero_when_at_start() {
        assert_eq!(w().effective_distance(100.0), 0.0);
    }

    #[test]
    fn effective_distance_scales_with_progress() {
        let mut w = w();
        w.progress = 75.0;
        assert!((w.effective_distance(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_distance_zero_when_disabled() {
        let mut w = w();
        w.progress = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_distance(100.0), 0.0);
    }
}
