use bevy_ecs::prelude::Component;

/// Distance tracker for a far-point or waypoint. Records how far away a
/// designated target is. `set(distance)` acquires a target; `close_in(amount)`
/// advances toward it. Models AI waypoints, objective markers, radar blip
/// tracking, or any mechanic where an entity must travel a measured distance
/// to reach a goal.
///
/// `set(distance)` updates tracked distance (clamped to [0, `max_range`]).
/// Fires `just_acquired` when a non-zero distance is set while at 0 (new
/// target spotted). Fires `just_arrived` when distance becomes 0 from a
/// non-zero value. No-op when disabled.
///
/// `close_in(amount)` decreases distance toward 0. Fires `just_arrived` on
/// reaching 0. No-op when disabled, already arrived, or `amount <= 0`.
///
/// `tick(_dt)` clears `just_acquired` and `just_arrived` only.
///
/// `is_active()` returns `distance > 0.0 && enabled`.
///
/// `is_arrived()` returns `distance == 0.0` (not gated by `enabled`).
///
/// `distance_fraction()` returns `(distance / max_range).clamp(0, 1)`.
///
/// `effective_pull(base)` returns `base * (1.0 - distance_fraction())` when
/// `is_active()`; `0.0` otherwise. Pull is strongest when nearly arrived.
///
/// Default: `new(100.0)` — no target acquired.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yonder {
    pub distance: f32,
    pub max_range: f32,
    pub just_acquired: bool,
    pub just_arrived: bool,
    pub enabled: bool,
}

impl Yonder {
    pub fn new(max_range: f32) -> Self {
        Self {
            distance: 0.0,
            max_range: max_range.max(0.1),
            just_acquired: false,
            just_arrived: false,
            enabled: true,
        }
    }

    /// Set target distance. Fires `just_acquired` when transitioning from no
    /// target to a new one, and `just_arrived` when set to 0 from non-zero.
    /// No-op when disabled.
    pub fn set(&mut self, distance: f32) {
        if !self.enabled {
            return;
        }
        let prev = self.distance;
        self.distance = distance.clamp(0.0, self.max_range);
        if prev == 0.0 && self.distance > 0.0 {
            self.just_acquired = true;
        } else if prev > 0.0 && self.distance == 0.0 {
            self.just_arrived = true;
        }
    }

    /// Advance toward target by `amount`. Fires `just_arrived` on reaching 0.
    /// No-op when disabled, already arrived, or `amount <= 0`.
    pub fn close_in(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.distance <= 0.0 {
            return;
        }
        self.distance = (self.distance - amount).max(0.0);
        if self.distance == 0.0 {
            self.just_arrived = true;
        }
    }

    /// Advance one frame: clear `just_acquired` and `just_arrived` only.
    pub fn tick(&mut self, _dt: f32) {
        self.just_acquired = false;
        self.just_arrived = false;
    }

    /// `true` when a target is tracked and component is enabled.
    pub fn is_active(&self) -> bool {
        self.distance > 0.0 && self.enabled
    }

    /// `true` when distance is 0 (not gated by `enabled`).
    pub fn is_arrived(&self) -> bool {
        self.distance == 0.0
    }

    /// Fraction of `max_range` remaining [0.0, 1.0].
    pub fn distance_fraction(&self) -> f32 {
        (self.distance / self.max_range).clamp(0.0, 1.0)
    }

    /// Returns `base * (1.0 - distance_fraction())` when active; `0.0` when
    /// no target or disabled. Strength increases as target nears.
    pub fn effective_pull(&self, base: f32) -> f32 {
        if !self.is_active() {
            return 0.0;
        }
        base * (1.0 - self.distance_fraction())
    }
}

impl Default for Yonder {
    fn default() -> Self {
        Self::new(100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yonder {
        Yonder::new(100.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_with_no_target() {
        let y = y();
        assert_eq!(y.distance, 0.0);
        assert!(y.is_arrived());
        assert!(!y.is_active());
    }

    #[test]
    fn new_clamps_max_range() {
        let y = Yonder::new(-5.0);
        assert!((y.max_range - 0.1).abs() < 1e-5);
    }

    #[test]
    fn default_max_range_is_hundred() {
        assert!((Yonder::default().max_range - 100.0).abs() < 1e-5);
    }

    // --- set ---

    #[test]
    fn set_updates_distance() {
        let mut y = y();
        y.set(50.0);
        assert!((y.distance - 50.0).abs() < 1e-4);
    }

    #[test]
    fn set_fires_just_acquired_from_zero() {
        let mut y = y();
        y.set(50.0);
        assert!(y.just_acquired);
    }

    #[test]
    fn set_clamps_to_max_range() {
        let mut y = y();
        y.set(200.0);
        assert!((y.distance - 100.0).abs() < 1e-5);
    }

    #[test]
    fn set_fires_just_arrived_when_zeroed() {
        let mut y = y();
        y.set(50.0);
        y.tick(0.016);
        y.set(0.0);
        assert!(y.just_arrived);
        assert!(y.is_arrived());
    }

    #[test]
    fn set_does_not_fire_acquired_when_updating_active() {
        let mut y = y();
        y.set(50.0);
        y.tick(0.016);
        y.set(30.0);
        assert!(!y.just_acquired); // was already active
    }

    #[test]
    fn set_does_not_fire_arrived_when_already_zero() {
        let mut y = y();
        y.set(0.0); // stays at 0
        assert!(!y.just_arrived);
    }

    #[test]
    fn set_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.set(50.0);
        assert_eq!(y.distance, 0.0);
    }

    // --- close_in ---

    #[test]
    fn close_in_reduces_distance() {
        let mut y = y();
        y.set(80.0);
        y.tick(0.016);
        y.close_in(20.0);
        assert!((y.distance - 60.0).abs() < 1e-4);
    }

    #[test]
    fn close_in_clamps_at_zero() {
        let mut y = y();
        y.set(30.0);
        y.tick(0.016);
        y.close_in(50.0);
        assert_eq!(y.distance, 0.0);
    }

    #[test]
    fn close_in_fires_just_arrived() {
        let mut y = y();
        y.set(30.0);
        y.tick(0.016);
        y.close_in(30.0);
        assert!(y.just_arrived);
    }

    #[test]
    fn close_in_no_op_when_already_arrived() {
        let mut y = y();
        y.close_in(10.0);
        assert!(!y.just_arrived);
    }

    #[test]
    fn close_in_no_op_when_disabled() {
        let mut y = y();
        y.set(50.0);
        y.enabled = false;
        y.close_in(20.0);
        assert!((y.distance - 50.0).abs() < 1e-4);
    }

    #[test]
    fn close_in_no_op_for_zero_amount() {
        let mut y = y();
        y.set(50.0);
        y.close_in(0.0);
        assert!((y.distance - 50.0).abs() < 1e-4);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_acquired() {
        let mut y = y();
        y.set(50.0);
        y.tick(0.016);
        assert!(!y.just_acquired);
    }

    #[test]
    fn tick_clears_just_arrived() {
        let mut y = y();
        y.set(30.0);
        y.close_in(30.0);
        y.tick(0.016);
        assert!(!y.just_arrived);
    }

    #[test]
    fn tick_does_not_change_distance() {
        let mut y = y();
        y.set(60.0);
        y.tick(1000.0);
        assert!((y.distance - 60.0).abs() < 1e-5);
    }

    // --- is_active / is_arrived ---

    #[test]
    fn is_active_true_with_distance() {
        let mut y = y();
        y.set(50.0);
        assert!(y.is_active());
    }

    #[test]
    fn is_active_false_at_zero() {
        assert!(!y().is_active());
    }

    #[test]
    fn is_active_false_when_disabled() {
        let mut y = y();
        y.set(50.0);
        y.enabled = false;
        assert!(!y.is_active());
    }

    #[test]
    fn is_arrived_true_at_zero() {
        assert!(y().is_arrived());
    }

    #[test]
    fn is_arrived_true_even_when_disabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_arrived()); // not gated
    }

    #[test]
    fn is_arrived_false_with_distance() {
        let mut y = y();
        y.set(50.0);
        assert!(!y.is_arrived());
    }

    // --- fractions / effective ---

    #[test]
    fn distance_fraction_zero_when_arrived() {
        assert_eq!(y().distance_fraction(), 0.0);
    }

    #[test]
    fn distance_fraction_half_at_midpoint() {
        let mut y = y();
        y.set(50.0);
        assert!((y.distance_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn distance_fraction_one_at_max_range() {
        let mut y = y();
        y.set(100.0);
        assert!((y.distance_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_pull_zero_when_no_target() {
        assert_eq!(y().effective_pull(100.0), 0.0);
    }

    #[test]
    fn effective_pull_max_when_at_zero_distance() {
        // Can't reach 0 with set(0) from 0 (no acquisition), so set then close
        let mut y = y();
        y.set(1.0);
        y.tick(0.016);
        y.close_in(0.5); // 0.5 remains
        assert!((y.effective_pull(100.0) - 99.5).abs() < 0.1);
    }

    #[test]
    fn effective_pull_zero_when_at_max_range() {
        let mut y = y();
        y.set(100.0);
        assert!((y.effective_pull(100.0) - 0.0).abs() < 1e-4); // 1 - 1.0 = 0
    }

    #[test]
    fn effective_pull_zero_when_disabled() {
        let mut y = y();
        y.set(50.0);
        y.enabled = false;
        assert_eq!(y.effective_pull(100.0), 0.0);
    }
}
