use bevy_ecs::prelude::Component;

/// Canopy-shade tree growth tracker. `canopy` builds via `branch(amount)`
/// and expands passively at `grow_rate` per second in `tick(dt)` or
/// is reduced immediately via `prune(amount)`.
///
/// Models urban-tree canopy coverage meters, parkland shade fill levels,
/// arboretum specimen growth gauges, reforestation progress trackers,
/// Asian-temperate-forest canopy expansion indicators, garden-elm maturity
/// bars, zelkova-avenue shading accumulators, street-tree canopy fill
/// levels, city-boulevard foliage density meters, or any mechanic where
/// a stately deciduous tree slowly expands its canopy to cast
/// maximum shade over a garden or urban space.
///
/// `branch(amount)` adds canopy; fires `just_canopied` when first
/// reaching `max_canopy`. No-op when disabled.
///
/// `prune(amount)` reduces canopy immediately; fires `just_bare`
/// when reaching 0. No-op when disabled or already bare.
///
/// `tick(dt)` clears both flags, then increases canopy by
/// `grow_rate * dt` (capped at `max_canopy`). Fires `just_canopied`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_canopied()` returns `canopy >= max_canopy && enabled`.
///
/// `is_bare()` returns `canopy == 0.0` (not gated by `enabled`).
///
/// `canopy_fraction()` returns `(canopy / max_canopy).clamp(0, 1)`.
///
/// `effective_shade(scale)` returns `scale * canopy_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — grows at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zelkova {
    pub canopy: f32,
    pub max_canopy: f32,
    pub grow_rate: f32,
    pub just_canopied: bool,
    pub just_bare: bool,
    pub enabled: bool,
}

impl Zelkova {
    pub fn new(max_canopy: f32, grow_rate: f32) -> Self {
        Self {
            canopy: 0.0,
            max_canopy: max_canopy.max(0.1),
            grow_rate: grow_rate.max(0.0),
            just_canopied: false,
            just_bare: false,
            enabled: true,
        }
    }

    /// Add canopy; fires `just_canopied` when first reaching max.
    /// No-op when disabled.
    pub fn branch(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.canopy < self.max_canopy;
        self.canopy = (self.canopy + amount).min(self.max_canopy);
        if was_below && self.canopy >= self.max_canopy {
            self.just_canopied = true;
        }
    }

    /// Reduce canopy; fires `just_bare` when reaching 0.
    /// No-op when disabled or already bare.
    pub fn prune(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.canopy <= 0.0 {
            return;
        }
        self.canopy = (self.canopy - amount).max(0.0);
        if self.canopy <= 0.0 {
            self.just_bare = true;
        }
    }

    /// Clear flags, then increase canopy by `grow_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_canopied = false;
        self.just_bare = false;
        if self.enabled && self.grow_rate > 0.0 && self.canopy < self.max_canopy {
            let was_below = self.canopy < self.max_canopy;
            self.canopy = (self.canopy + self.grow_rate * dt).min(self.max_canopy);
            if was_below && self.canopy >= self.max_canopy {
                self.just_canopied = true;
            }
        }
    }

    /// `true` when canopy is at maximum and component is enabled.
    pub fn is_canopied(&self) -> bool {
        self.canopy >= self.max_canopy && self.enabled
    }

    /// `true` when canopy is 0 (not gated by `enabled`).
    pub fn is_bare(&self) -> bool {
        self.canopy == 0.0
    }

    /// Fraction of maximum canopy [0.0, 1.0].
    pub fn canopy_fraction(&self) -> f32 {
        (self.canopy / self.max_canopy).clamp(0.0, 1.0)
    }

    /// Returns `scale * canopy_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_shade(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.canopy_fraction()
    }
}

impl Default for Zelkova {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zelkova {
        Zelkova::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_bare() {
        let z = z();
        assert_eq!(z.canopy, 0.0);
        assert!(z.is_bare());
        assert!(!z.is_canopied());
    }

    #[test]
    fn new_clamps_max_canopy() {
        let z = Zelkova::new(-5.0, 1.5);
        assert!((z.max_canopy - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_grow_rate() {
        let z = Zelkova::new(100.0, -3.0);
        assert_eq!(z.grow_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zelkova::default();
        assert!((z.max_canopy - 100.0).abs() < 1e-5);
        assert!((z.grow_rate - 1.5).abs() < 1e-5);
    }

    // --- branch ---

    #[test]
    fn branch_adds_canopy() {
        let mut z = z();
        z.branch(40.0);
        assert!((z.canopy - 40.0).abs() < 1e-3);
    }

    #[test]
    fn branch_clamps_at_max() {
        let mut z = z();
        z.branch(200.0);
        assert!((z.canopy - 100.0).abs() < 1e-3);
    }

    #[test]
    fn branch_fires_just_canopied_at_max() {
        let mut z = z();
        z.branch(100.0);
        assert!(z.just_canopied);
        assert!(z.is_canopied());
    }

    #[test]
    fn branch_no_just_canopied_when_already_at_max() {
        let mut z = z();
        z.canopy = 100.0;
        z.branch(10.0);
        assert!(!z.just_canopied);
    }

    #[test]
    fn branch_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.branch(50.0);
        assert_eq!(z.canopy, 0.0);
    }

    #[test]
    fn branch_no_op_when_amount_zero() {
        let mut z = z();
        z.branch(0.0);
        assert_eq!(z.canopy, 0.0);
    }

    // --- prune ---

    #[test]
    fn prune_reduces_canopy() {
        let mut z = z();
        z.canopy = 60.0;
        z.prune(20.0);
        assert!((z.canopy - 40.0).abs() < 1e-3);
    }

    #[test]
    fn prune_clamps_at_zero() {
        let mut z = z();
        z.canopy = 30.0;
        z.prune(200.0);
        assert_eq!(z.canopy, 0.0);
    }

    #[test]
    fn prune_fires_just_bare_at_zero() {
        let mut z = z();
        z.canopy = 30.0;
        z.prune(30.0);
        assert!(z.just_bare);
    }

    #[test]
    fn prune_no_op_when_already_bare() {
        let mut z = z();
        z.prune(10.0);
        assert!(!z.just_bare);
    }

    #[test]
    fn prune_no_op_when_disabled() {
        let mut z = z();
        z.canopy = 50.0;
        z.enabled = false;
        z.prune(50.0);
        assert!((z.canopy - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_grows_canopy() {
        let mut z = z(); // rate=1.5
        z.tick(2.0); // 0 + 1.5*2 = 3
        assert!((z.canopy - 3.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_canopied_on_grow_to_max() {
        let mut z = Zelkova::new(100.0, 200.0);
        z.canopy = 95.0;
        z.tick(1.0);
        assert!(z.just_canopied);
        assert!(z.is_canopied());
    }

    #[test]
    fn tick_no_grow_when_already_canopied() {
        let mut z = z();
        z.canopy = 100.0;
        z.tick(1.0);
        assert!(!z.just_canopied);
    }

    #[test]
    fn tick_no_grow_when_rate_zero() {
        let mut z = Zelkova::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.canopy, 0.0);
    }

    #[test]
    fn tick_no_grow_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.canopy, 0.0);
    }

    #[test]
    fn tick_clears_just_canopied() {
        let mut z = Zelkova::new(100.0, 200.0);
        z.canopy = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_canopied);
    }

    #[test]
    fn tick_clears_just_bare() {
        let mut z = z();
        z.canopy = 10.0;
        z.prune(10.0);
        z.tick(0.016);
        assert!(!z.just_bare);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 1.5*4 = 6
        assert!((z.canopy - 6.0).abs() < 1e-3);
    }

    // --- is_canopied / is_bare ---

    #[test]
    fn is_canopied_false_when_disabled() {
        let mut z = z();
        z.canopy = 100.0;
        z.enabled = false;
        assert!(!z.is_canopied());
    }

    #[test]
    fn is_bare_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_bare());
    }

    // --- canopy_fraction / effective_shade ---

    #[test]
    fn canopy_fraction_zero_when_bare() {
        assert_eq!(z().canopy_fraction(), 0.0);
    }

    #[test]
    fn canopy_fraction_half_at_midpoint() {
        let mut z = z();
        z.canopy = 50.0;
        assert!((z.canopy_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_shade_zero_when_bare() {
        assert_eq!(z().effective_shade(100.0), 0.0);
    }

    #[test]
    fn effective_shade_scales_with_canopy() {
        let mut z = z();
        z.canopy = 80.0;
        assert!((z.effective_shade(100.0) - 80.0).abs() < 1e-3);
    }

    #[test]
    fn effective_shade_zero_when_disabled() {
        let mut z = z();
        z.canopy = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_shade(100.0), 0.0);
    }
}
