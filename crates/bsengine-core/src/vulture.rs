use bevy_ecs::prelude::Component;

/// Carrion-patience accumulation tracker named after vulture, the
/// common noun for any of several large birds of prey that feed
/// chiefly on carrion and are characterised by naked or sparsely
/// feathered heads, broad wings suited to thermal soaring, and keen
/// eyesight that allows them to spot potential food from great
/// altitudes — from the Latin vultur, possibly related to vellere
/// (to pluck, to tear). The two groups of living vultures —
/// Old World vultures (family Accipitridae) and New World vultures
/// (family Cathartidae) — arrived at their ecological niche through
/// convergent evolution: they are not closely related but evolved
/// the same morphological solutions to the same problem, which is
/// how to exploit the abundant energy in large dead animals without
/// competing directly with the faster predators that make the kills.
/// The vulture's strategy is patience and altitude: circle high
/// enough to scan a large area, wait long enough for competitors
/// to exhaust themselves over the carcass, then descend in numbers
/// large enough that no single rival can monopolise the resource.
/// The species became the archetype of patient opportunism in human
/// thought: the vulture circling overhead became the symbol of
/// anticipated death, of waiting for another's fall, of the patience
/// that does not create but positions itself to exploit. The vulture
/// loan shark, the vulture fund, the vulture journalist — all share
/// the structure of an entity that waits for weakness to reach its
/// nadir before moving in to claim whatever value remains. In game
/// mechanics, a vulture mechanic models the accumulation of patient
/// waiting — the slow build of circling patience or opportunistic
/// positioning that eventually enables a devastating strike on a
/// weakened or exposed target. `patience` builds via
/// `circle(amount)` and accumulates passively at `wait_rate` per
/// second in `tick(dt)` or resets via `scatter(amount)`.
///
/// Models carrion-patience fill levels, opportunistic-circling
/// saturation bars, scavenger-positioning accumulators, patient-
/// wait gauges, aerial-surveillance fill levels, deathwatch
/// saturation indicators, opportunism-build accumulation bars,
/// scavenger-approach meters, circling-patience fill levels, or
/// any mechanic where a creature, faction, or system slowly
/// accumulates the patience and positioning required to exploit
/// a weakened opponent — circling higher and higher until the
/// target's defences collapse and the opportunistic strike
/// can finally be made with maximum effect and minimum risk.
///
/// `circle(amount)` adds patience; fires `just_descended` when
/// first reaching `max_patience`. No-op when disabled.
///
/// `scatter(amount)` reduces patience immediately; fires
/// `just_scattered` when reaching 0. No-op when disabled or
/// already scattered.
///
/// `tick(dt)` clears both flags, then increases patience by
/// `wait_rate * dt` (capped at `max_patience`). Fires
/// `just_descended` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_descended()` returns `patience >= max_patience && enabled`.
///
/// `is_scattered()` returns `patience == 0.0` (not gated by
/// `enabled`).
///
/// `patience_fraction()` returns
/// `(patience / max_patience).clamp(0, 1)`.
///
/// `effective_scavenge(scale)` returns `scale * patience_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — circles at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Vulture {
    pub patience: f32,
    pub max_patience: f32,
    pub wait_rate: f32,
    pub just_descended: bool,
    pub just_scattered: bool,
    pub enabled: bool,
}

impl Vulture {
    pub fn new(max_patience: f32, wait_rate: f32) -> Self {
        Self {
            patience: 0.0,
            max_patience: max_patience.max(0.1),
            wait_rate: wait_rate.max(0.0),
            just_descended: false,
            just_scattered: false,
            enabled: true,
        }
    }

    /// Add patience; fires `just_descended` when first reaching max.
    /// No-op when disabled.
    pub fn circle(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.patience < self.max_patience;
        self.patience = (self.patience + amount).min(self.max_patience);
        if was_below && self.patience >= self.max_patience {
            self.just_descended = true;
        }
    }

    /// Reduce patience; fires `just_scattered` when reaching 0.
    /// No-op when disabled or already scattered.
    pub fn scatter(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.patience <= 0.0 {
            return;
        }
        self.patience = (self.patience - amount).max(0.0);
        if self.patience <= 0.0 {
            self.just_scattered = true;
        }
    }

    /// Clear flags, then increase patience by `wait_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_descended = false;
        self.just_scattered = false;
        if self.enabled && self.wait_rate > 0.0 && self.patience < self.max_patience {
            let was_below = self.patience < self.max_patience;
            self.patience = (self.patience + self.wait_rate * dt).min(self.max_patience);
            if was_below && self.patience >= self.max_patience {
                self.just_descended = true;
            }
        }
    }

    /// `true` when patience is at maximum and component is enabled.
    pub fn is_descended(&self) -> bool {
        self.patience >= self.max_patience && self.enabled
    }

    /// `true` when patience is 0 (not gated by `enabled`).
    pub fn is_scattered(&self) -> bool {
        self.patience == 0.0
    }

    /// Fraction of maximum patience [0.0, 1.0].
    pub fn patience_fraction(&self) -> f32 {
        (self.patience / self.max_patience).clamp(0.0, 1.0)
    }

    /// Returns `scale * patience_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_scavenge(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.patience_fraction()
    }
}

impl Default for Vulture {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v() -> Vulture {
        Vulture::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_scattered() {
        let v = v();
        assert_eq!(v.patience, 0.0);
        assert!(v.is_scattered());
        assert!(!v.is_descended());
    }

    #[test]
    fn new_clamps_max_patience() {
        let v = Vulture::new(-5.0, 1.5);
        assert!((v.max_patience - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_wait_rate() {
        let v = Vulture::new(100.0, -1.5);
        assert_eq!(v.wait_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let v = Vulture::default();
        assert!((v.max_patience - 100.0).abs() < 1e-5);
        assert!((v.wait_rate - 1.5).abs() < 1e-5);
    }

    // --- circle ---

    #[test]
    fn circle_adds_patience() {
        let mut v = v();
        v.circle(40.0);
        assert!((v.patience - 40.0).abs() < 1e-3);
    }

    #[test]
    fn circle_clamps_at_max() {
        let mut v = v();
        v.circle(200.0);
        assert!((v.patience - 100.0).abs() < 1e-3);
    }

    #[test]
    fn circle_fires_just_descended_at_max() {
        let mut v = v();
        v.circle(100.0);
        assert!(v.just_descended);
        assert!(v.is_descended());
    }

    #[test]
    fn circle_no_just_descended_when_already_at_max() {
        let mut v = v();
        v.patience = 100.0;
        v.circle(10.0);
        assert!(!v.just_descended);
    }

    #[test]
    fn circle_no_op_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.circle(50.0);
        assert_eq!(v.patience, 0.0);
    }

    #[test]
    fn circle_no_op_when_amount_zero() {
        let mut v = v();
        v.circle(0.0);
        assert_eq!(v.patience, 0.0);
    }

    // --- scatter ---

    #[test]
    fn scatter_reduces_patience() {
        let mut v = v();
        v.patience = 60.0;
        v.scatter(20.0);
        assert!((v.patience - 40.0).abs() < 1e-3);
    }

    #[test]
    fn scatter_clamps_at_zero() {
        let mut v = v();
        v.patience = 30.0;
        v.scatter(200.0);
        assert_eq!(v.patience, 0.0);
    }

    #[test]
    fn scatter_fires_just_scattered_at_zero() {
        let mut v = v();
        v.patience = 30.0;
        v.scatter(30.0);
        assert!(v.just_scattered);
    }

    #[test]
    fn scatter_no_op_when_already_scattered() {
        let mut v = v();
        v.scatter(10.0);
        assert!(!v.just_scattered);
    }

    #[test]
    fn scatter_no_op_when_disabled() {
        let mut v = v();
        v.patience = 50.0;
        v.enabled = false;
        v.scatter(50.0);
        assert!((v.patience - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_patience() {
        let mut v = v(); // rate=1.5
        v.tick(4.0); // 0 + 1.5*4 = 6
        assert!((v.patience - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_descended_on_patience_to_max() {
        let mut v = Vulture::new(100.0, 200.0);
        v.patience = 95.0;
        v.tick(1.0);
        assert!(v.just_descended);
        assert!(v.is_descended());
    }

    #[test]
    fn tick_no_build_when_already_descended() {
        let mut v = v();
        v.patience = 100.0;
        v.tick(1.0);
        assert!(!v.just_descended);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut v = Vulture::new(100.0, 0.0);
        v.tick(100.0);
        assert_eq!(v.patience, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.tick(1.0);
        assert_eq!(v.patience, 0.0);
    }

    #[test]
    fn tick_clears_just_descended() {
        let mut v = Vulture::new(100.0, 200.0);
        v.patience = 95.0;
        v.tick(1.0);
        v.tick(0.016);
        assert!(!v.just_descended);
    }

    #[test]
    fn tick_clears_just_scattered() {
        let mut v = v();
        v.patience = 10.0;
        v.scatter(10.0);
        v.tick(0.016);
        assert!(!v.just_scattered);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut v = v(); // rate=1.5
        v.tick(6.0); // 1.5*6 = 9
        assert!((v.patience - 9.0).abs() < 1e-3);
    }

    // --- is_descended / is_scattered ---

    #[test]
    fn is_descended_false_when_disabled() {
        let mut v = v();
        v.patience = 100.0;
        v.enabled = false;
        assert!(!v.is_descended());
    }

    #[test]
    fn is_scattered_not_gated_by_enabled() {
        let mut v = v();
        v.enabled = false;
        assert!(v.is_scattered());
    }

    // --- patience_fraction / effective_scavenge ---

    #[test]
    fn patience_fraction_zero_when_scattered() {
        assert_eq!(v().patience_fraction(), 0.0);
    }

    #[test]
    fn patience_fraction_half_at_midpoint() {
        let mut v = v();
        v.patience = 50.0;
        assert!((v.patience_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_scavenge_zero_when_scattered() {
        assert_eq!(v().effective_scavenge(100.0), 0.0);
    }

    #[test]
    fn effective_scavenge_scales_with_patience() {
        let mut v = v();
        v.patience = 75.0;
        assert!((v.effective_scavenge(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_scavenge_zero_when_disabled() {
        let mut v = v();
        v.patience = 50.0;
        v.enabled = false;
        assert_eq!(v.effective_scavenge(100.0), 0.0);
    }
}
