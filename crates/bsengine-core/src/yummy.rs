use bevy_ecs::prelude::Component;

/// Appeal and desirability tracker. `appeal` accumulates via `flavor(amount)`
/// and degrades passively at `spoil_rate` per second via `tick(dt)`. Active
/// spoilage is available via `spoil(amount)`.
///
/// Models resource desirability for AI pathfinding (NPCs prefer high-appeal
/// targets), loot attractiveness ratings, social popularity meters, or any
/// mechanic where a target becomes more or less desirable over time.
///
/// `flavor(amount)` adds to appeal (capped at `max_appeal`). Fires
/// `just_irresistible` on first reaching max. No-op when disabled.
///
/// `spoil(amount)` reduces appeal when above 0. Fires `just_spoiled` when
/// appeal reaches 0. No-op when disabled.
///
/// `tick(dt)` clears `just_irresistible` and `just_spoiled`. Then (when
/// enabled and `spoil_rate > 0`) reduces appeal by `spoil_rate * dt`, floored
/// at 0. Fires `just_spoiled` if appeal reaches 0 via spoilage.
///
/// `is_irresistible()` returns `appeal >= max_appeal && enabled`.
///
/// `is_spoiled()` returns `appeal == 0.0` (not gated by `enabled`).
///
/// `appeal_fraction()` returns `(appeal / max_appeal).clamp(0, 1)`.
///
/// `effective_attraction(base)` returns `base * appeal_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — starts unappealing, spoils at 2/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yummy {
    pub appeal: f32,
    pub max_appeal: f32,
    pub spoil_rate: f32,
    pub just_irresistible: bool,
    pub just_spoiled: bool,
    pub enabled: bool,
}

impl Yummy {
    pub fn new(max_appeal: f32, spoil_rate: f32) -> Self {
        Self {
            appeal: 0.0,
            max_appeal: max_appeal.max(0.1),
            spoil_rate: spoil_rate.max(0.0),
            just_irresistible: false,
            just_spoiled: false,
            enabled: true,
        }
    }

    /// Add appeal; fires `just_irresistible` on first reaching max.
    /// No-op when disabled or already at max.
    pub fn flavor(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.appeal >= self.max_appeal {
            return;
        }
        self.appeal = (self.appeal + amount).min(self.max_appeal);
        if self.appeal >= self.max_appeal {
            self.just_irresistible = true;
        }
    }

    /// Degrade appeal; fires `just_spoiled` when reaching 0.
    /// No-op when disabled or already spoiled.
    pub fn spoil(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.appeal <= 0.0 {
            return;
        }
        self.appeal = (self.appeal - amount).max(0.0);
        if self.appeal <= 0.0 {
            self.just_spoiled = true;
        }
    }

    /// Advance one frame: clear flags, then degrade appeal passively when
    /// enabled and `spoil_rate > 0`. Fires `just_spoiled` if appeal hits 0.
    pub fn tick(&mut self, dt: f32) {
        self.just_irresistible = false;
        self.just_spoiled = false;
        if self.enabled && self.spoil_rate > 0.0 && self.appeal > 0.0 {
            self.appeal = (self.appeal - self.spoil_rate * dt).max(0.0);
            if self.appeal <= 0.0 {
                self.just_spoiled = true;
            }
        }
    }

    /// `true` when appeal is at maximum and component is enabled.
    pub fn is_irresistible(&self) -> bool {
        self.appeal >= self.max_appeal && self.enabled
    }

    /// `true` when appeal is 0 (not gated by `enabled`).
    pub fn is_spoiled(&self) -> bool {
        self.appeal == 0.0
    }

    /// Fraction of maximum appeal [0.0, 1.0].
    pub fn appeal_fraction(&self) -> f32 {
        (self.appeal / self.max_appeal).clamp(0.0, 1.0)
    }

    /// Returns `base * appeal_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_attraction(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.appeal_fraction()
    }
}

impl Default for Yummy {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yummy {
        Yummy::new(100.0, 10.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_spoiled() {
        let y = y();
        assert_eq!(y.appeal, 0.0);
        assert!(y.is_spoiled());
        assert!(!y.is_irresistible());
    }

    #[test]
    fn new_clamps_max_appeal() {
        let y = Yummy::new(-5.0, 0.0);
        assert!((y.max_appeal - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_spoil_rate() {
        let y = Yummy::new(100.0, -3.0);
        assert_eq!(y.spoil_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let y = Yummy::default();
        assert!((y.max_appeal - 100.0).abs() < 1e-5);
        assert!((y.spoil_rate - 2.0).abs() < 1e-5);
        assert_eq!(y.appeal, 0.0);
    }

    // --- flavor ---

    #[test]
    fn flavor_increases_appeal() {
        let mut y = y();
        y.flavor(40.0);
        assert!((y.appeal - 40.0).abs() < 1e-4);
    }

    #[test]
    fn flavor_clamps_at_max() {
        let mut y = y();
        y.flavor(200.0);
        assert!((y.appeal - 100.0).abs() < 1e-5);
    }

    #[test]
    fn flavor_fires_just_irresistible_at_max() {
        let mut y = y();
        y.flavor(100.0);
        assert!(y.just_irresistible);
        assert!(y.is_irresistible());
    }

    #[test]
    fn flavor_no_refire_when_at_max() {
        let mut y = y();
        y.flavor(100.0);
        y.flavor(10.0); // already at max
        assert!(y.just_irresistible);
    }

    #[test]
    fn flavor_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.flavor(50.0);
        assert_eq!(y.appeal, 0.0);
    }

    #[test]
    fn flavor_no_op_for_zero() {
        let mut y = y();
        y.flavor(0.0);
        assert_eq!(y.appeal, 0.0);
    }

    #[test]
    fn flavor_accumulates() {
        let mut y = y();
        y.flavor(30.0);
        y.flavor(25.0);
        assert!((y.appeal - 55.0).abs() < 1e-3);
    }

    // --- spoil ---

    #[test]
    fn spoil_reduces_appeal() {
        let mut y = y();
        y.flavor(70.0);
        y.spoil(20.0);
        assert!((y.appeal - 50.0).abs() < 1e-3);
    }

    #[test]
    fn spoil_clamps_at_zero() {
        let mut y = y();
        y.flavor(30.0);
        y.spoil(200.0);
        assert_eq!(y.appeal, 0.0);
    }

    #[test]
    fn spoil_fires_just_spoiled_at_zero() {
        let mut y = y();
        y.flavor(30.0);
        y.spoil(30.0);
        assert!(y.just_spoiled);
        assert!(y.is_spoiled());
    }

    #[test]
    fn spoil_no_op_when_already_spoiled() {
        let mut y = y();
        y.spoil(10.0); // already 0
        assert!(!y.just_spoiled);
    }

    #[test]
    fn spoil_no_op_when_disabled() {
        let mut y = y();
        y.flavor(50.0);
        y.enabled = false;
        y.spoil(50.0);
        assert!((y.appeal - 50.0).abs() < 1e-3);
    }

    // --- tick (passive spoilage) ---

    #[test]
    fn tick_degrades_appeal() {
        let mut y = y(); // spoil_rate = 10
        y.flavor(60.0);
        y.tick(1.0); // 60 - 10 = 50
        assert!((y.appeal - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clamps_at_zero() {
        let mut y = y();
        y.flavor(5.0);
        y.tick(100.0);
        assert_eq!(y.appeal, 0.0);
    }

    #[test]
    fn tick_fires_just_spoiled_on_reaching_zero() {
        let mut y = y();
        y.flavor(5.0);
        y.tick(1.0); // drains 10 → 0
        assert!(y.just_spoiled);
    }

    #[test]
    fn tick_no_spoilage_when_already_spoiled() {
        let mut y = y();
        y.tick(100.0); // appeal=0
        assert!(!y.just_spoiled);
    }

    #[test]
    fn tick_no_spoilage_when_rate_zero() {
        let mut y = Yummy::new(100.0, 0.0);
        y.flavor(50.0);
        y.tick(100.0);
        assert!((y.appeal - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_spoilage_when_disabled() {
        let mut y = y();
        y.flavor(50.0);
        y.enabled = false;
        y.tick(1.0);
        assert!((y.appeal - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_irresistible() {
        let mut y = y();
        y.flavor(100.0);
        y.tick(0.016);
        assert!(!y.just_irresistible);
    }

    #[test]
    fn tick_clears_just_spoiled() {
        let mut y = y();
        y.flavor(5.0);
        y.tick(1.0); // just_spoiled fires
        y.tick(0.016); // cleared
        assert!(!y.just_spoiled);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut y = y();
        y.flavor(80.0);
        y.tick(2.0); // 80 - 10*2 = 60
        assert!((y.appeal - 60.0).abs() < 1e-2);
    }

    // --- is_irresistible / is_spoiled ---

    #[test]
    fn is_irresistible_false_below_max() {
        let mut y = y();
        y.flavor(50.0);
        assert!(!y.is_irresistible());
    }

    #[test]
    fn is_irresistible_false_when_disabled() {
        let mut y = y();
        y.flavor(100.0);
        y.enabled = false;
        assert!(!y.is_irresistible());
    }

    #[test]
    fn is_spoiled_true_at_start() {
        assert!(y().is_spoiled());
    }

    #[test]
    fn is_spoiled_not_gated_by_enabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_spoiled());
    }

    // --- fractions / effective ---

    #[test]
    fn appeal_fraction_zero_when_spoiled() {
        assert_eq!(y().appeal_fraction(), 0.0);
    }

    #[test]
    fn appeal_fraction_half_at_midpoint() {
        let mut y = y();
        y.flavor(50.0);
        assert!((y.appeal_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_attraction_zero_when_spoiled() {
        assert_eq!(y().effective_attraction(100.0), 0.0);
    }

    #[test]
    fn effective_attraction_scales_with_fraction() {
        let mut y = y();
        y.flavor(75.0);
        assert!((y.effective_attraction(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_attraction_zero_when_disabled() {
        let mut y = y();
        y.flavor(50.0);
        y.enabled = false;
        assert_eq!(y.effective_attraction(100.0), 0.0);
    }
}
