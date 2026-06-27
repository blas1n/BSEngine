use bevy_ecs::prelude::Component;

/// Territorial boundary tracker. Counts how many entities have entered this
/// entity's yard (defensive perimeter) and fires events when the boundary is
/// first breached or finally cleared. Models guard zones, aggro radii, personal
/// space, or property-ownership mechanics.
///
/// `breach()` registers one entity entering the yard. Fires `just_breached`
/// on the first entry. No-op when disabled.
///
/// `vacate()` removes one entity. Fires `just_cleared` when the last entity
/// leaves. No-op when disabled or already empty.
///
/// `vacate_all()` removes all intruders regardless of `enabled`. Fires
/// `just_cleared` if any were present.
///
/// `tick(_dt)` clears one-frame flags only.
///
/// `is_breached()` returns `intruder_count > 0 && enabled`.
///
/// `intruder_fraction(capacity)` returns `(intruder_count as f32 /
/// capacity as f32).clamp(0.0, 1.0)`. Returns `0.0` when capacity is 0.
///
/// `effective_threat(base)` returns `base` when `is_breached()`; `0.0`
/// otherwise.
///
/// Default: `new(10.0)` — 10-unit range.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yard {
    /// Nominal range of the territory boundary in world units.
    pub range: f32,
    /// Number of entities currently inside the yard.
    pub intruder_count: u32,
    pub just_breached: bool,
    pub just_cleared: bool,
    pub enabled: bool,
}

impl Yard {
    pub fn new(range: f32) -> Self {
        Self {
            range: range.max(0.0),
            intruder_count: 0,
            just_breached: false,
            just_cleared: false,
            enabled: true,
        }
    }

    /// Register one entity entering the yard. Fires `just_breached` on first
    /// entry. No-op when disabled.
    pub fn breach(&mut self) {
        if !self.enabled {
            return;
        }
        if self.intruder_count == 0 {
            self.just_breached = true;
        }
        self.intruder_count += 1;
    }

    /// Remove one entity from the yard. Fires `just_cleared` when the last
    /// intruder leaves. No-op when disabled or empty.
    pub fn vacate(&mut self) {
        if !self.enabled || self.intruder_count == 0 {
            return;
        }
        self.intruder_count -= 1;
        if self.intruder_count == 0 {
            self.just_cleared = true;
        }
    }

    /// Remove all intruders regardless of `enabled`. Fires `just_cleared` if
    /// any were present.
    pub fn vacate_all(&mut self) {
        if self.intruder_count > 0 {
            self.intruder_count = 0;
            self.just_cleared = true;
        }
    }

    /// Advance one frame: clear one-frame flags only.
    pub fn tick(&mut self, _dt: f32) {
        self.just_breached = false;
        self.just_cleared = false;
    }

    /// `true` when at least one intruder is present and component is enabled.
    pub fn is_breached(&self) -> bool {
        self.intruder_count > 0 && self.enabled
    }

    /// Intruder count as a fraction of `capacity` [0.0, 1.0]. `0.0` when
    /// capacity is 0.
    pub fn intruder_fraction(&self, capacity: u32) -> f32 {
        if capacity == 0 {
            return 0.0;
        }
        (self.intruder_count as f32 / capacity as f32).clamp(0.0, 1.0)
    }

    /// Returns `base` when the yard is breached and enabled; `0.0` otherwise.
    pub fn effective_threat(&self, base: f32) -> f32 {
        if self.is_breached() {
            base
        } else {
            0.0
        }
    }
}

impl Default for Yard {
    fn default() -> Self {
        Self::new(10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yard {
        Yard::new(10.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_empty() {
        let y = y();
        assert_eq!(y.intruder_count, 0);
        assert!(!y.just_breached);
        assert!(!y.just_cleared);
        assert!(!y.is_breached());
    }

    #[test]
    fn range_clamped_to_zero() {
        let y = Yard::new(-5.0);
        assert_eq!(y.range, 0.0);
    }

    #[test]
    fn default_range() {
        let y = Yard::default();
        assert!((y.range - 10.0).abs() < 1e-5);
    }

    // --- breach ---

    #[test]
    fn breach_increments_count() {
        let mut y = y();
        y.breach();
        assert_eq!(y.intruder_count, 1);
    }

    #[test]
    fn breach_fires_just_breached_on_first_entry() {
        let mut y = y();
        y.breach();
        assert!(y.just_breached);
        assert!(y.is_breached());
    }

    #[test]
    fn breach_does_not_refire_just_breached_on_second_entry() {
        let mut y = y();
        y.breach();
        y.tick(0.016);
        y.breach();
        assert!(!y.just_breached);
        assert_eq!(y.intruder_count, 2);
    }

    #[test]
    fn breach_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.breach();
        assert_eq!(y.intruder_count, 0);
        assert!(!y.just_breached);
    }

    // --- vacate ---

    #[test]
    fn vacate_decrements_count() {
        let mut y = y();
        y.breach();
        y.breach();
        y.tick(0.016);
        y.vacate();
        assert_eq!(y.intruder_count, 1);
    }

    #[test]
    fn vacate_fires_just_cleared_on_last_departure() {
        let mut y = y();
        y.breach();
        y.tick(0.016);
        y.vacate();
        assert!(y.just_cleared);
        assert!(!y.is_breached());
    }

    #[test]
    fn vacate_does_not_fire_just_cleared_while_others_remain() {
        let mut y = y();
        y.breach();
        y.breach();
        y.tick(0.016);
        y.vacate();
        assert!(!y.just_cleared);
        assert_eq!(y.intruder_count, 1);
    }

    #[test]
    fn vacate_no_op_when_empty() {
        let mut y = y();
        y.vacate();
        assert!(!y.just_cleared);
        assert_eq!(y.intruder_count, 0);
    }

    #[test]
    fn vacate_no_op_when_disabled() {
        let mut y = y();
        y.breach();
        y.enabled = false;
        y.vacate();
        assert_eq!(y.intruder_count, 1);
        assert!(!y.just_cleared);
    }

    // --- vacate_all ---

    #[test]
    fn vacate_all_clears_all_intruders() {
        let mut y = y();
        y.breach();
        y.breach();
        y.breach();
        y.vacate_all();
        assert_eq!(y.intruder_count, 0);
        assert!(y.just_cleared);
    }

    #[test]
    fn vacate_all_works_when_disabled() {
        let mut y = y();
        y.breach();
        y.enabled = false;
        y.vacate_all();
        assert_eq!(y.intruder_count, 0);
        assert!(y.just_cleared);
    }

    #[test]
    fn vacate_all_no_just_cleared_when_already_empty() {
        let mut y = y();
        y.vacate_all();
        assert!(!y.just_cleared);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_breached() {
        let mut y = y();
        y.breach();
        y.tick(0.016);
        assert!(!y.just_breached);
    }

    #[test]
    fn tick_clears_just_cleared() {
        let mut y = y();
        y.breach();
        y.vacate();
        y.tick(0.016);
        assert!(!y.just_cleared);
    }

    #[test]
    fn tick_does_not_change_count() {
        let mut y = y();
        y.breach();
        y.breach();
        y.tick(1000.0);
        assert_eq!(y.intruder_count, 2);
    }

    // --- is_breached ---

    #[test]
    fn is_breached_false_when_empty() {
        assert!(!y().is_breached());
    }

    #[test]
    fn is_breached_true_with_intruders() {
        let mut y = y();
        y.breach();
        assert!(y.is_breached());
    }

    #[test]
    fn is_breached_false_when_disabled() {
        let mut y = y();
        y.breach();
        y.enabled = false;
        assert!(!y.is_breached());
    }

    // --- intruder_fraction ---

    #[test]
    fn intruder_fraction_zero_when_empty() {
        assert_eq!(y().intruder_fraction(10), 0.0);
    }

    #[test]
    fn intruder_fraction_at_capacity() {
        let mut y = y();
        for _ in 0..5 {
            y.breach();
        }
        assert!((y.intruder_fraction(5) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn intruder_fraction_clamps_above_capacity() {
        let mut y = y();
        for _ in 0..10 {
            y.breach();
        }
        assert!((y.intruder_fraction(5) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn intruder_fraction_zero_when_capacity_zero() {
        let mut y = y();
        y.breach();
        assert_eq!(y.intruder_fraction(0), 0.0);
    }

    // --- effective_threat ---

    #[test]
    fn effective_threat_base_when_breached() {
        let mut y = y();
        y.breach();
        assert!((y.effective_threat(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_threat_zero_when_empty() {
        assert_eq!(y().effective_threat(100.0), 0.0);
    }

    #[test]
    fn effective_threat_zero_when_disabled() {
        let mut y = y();
        y.breach();
        y.enabled = false;
        assert_eq!(y.effective_threat(100.0), 0.0);
    }

    // --- breach-vacate cycle ---

    #[test]
    fn full_breach_and_clear_cycle() {
        let mut y = y();
        y.breach();
        assert!(y.just_breached);
        y.tick(0.016);
        y.vacate();
        assert!(y.just_cleared);
        y.tick(0.016);
        y.breach();
        assert!(y.just_breached); // can re-breach after clearing
    }
}
