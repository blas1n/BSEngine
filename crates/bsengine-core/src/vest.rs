use bevy_ecs::prelude::Component;

/// Flat per-hit damage absorber: each incoming hit has up to
/// `absorption_per_hit` removed before it reaches the entity. Unlike
/// percentage-based reductions, the absorption is capped at the incoming
/// damage (a 3-damage hit against a 5-absorption vest passes 0 damage
/// through).
///
/// `absorb(incoming)` subtracts `absorption_per_hit` from `incoming`
/// (floored at 0.0); records `last_absorbed`; increments `hits_absorbed`;
/// fires `just_absorbed` when any absorption occurred. No-op when disabled —
/// returns `incoming` unmodified.
///
/// `tick(dt)` clears `just_absorbed`. No-op when disabled.
///
/// `is_absorbing()` returns `absorption_per_hit > 0.0 && enabled`.
///
/// `effective_incoming(base)` is a **pure query** (does not mutate state):
/// returns `(base - absorption_per_hit).max(0.0)` when enabled; `base`
/// otherwise.
///
/// Distinct from `Barrier` (finite shield pool that depletes with each hit),
/// `Block` (chance-based full negation), `Armor` (percentage reduction), and
/// `Toughness` (flat reduction that always applies regardless of hit size):
/// Vest is a **per-hit flat absorber** — each hit independently loses up to
/// `absorption_per_hit` with no pool to exhaust, but also no percentage
/// scaling.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Vest {
    /// Flat damage absorbed per incoming hit. Clamped >= 0.0.
    pub absorption_per_hit: f32,
    /// Total number of hits where absorption occurred.
    pub hits_absorbed: u32,
    /// Amount absorbed on the most recent `absorb()` call.
    pub last_absorbed: f32,
    pub just_absorbed: bool,
    pub enabled: bool,
}

impl Vest {
    pub fn new(absorption_per_hit: f32) -> Self {
        Self {
            absorption_per_hit: absorption_per_hit.max(0.0),
            hits_absorbed: 0,
            last_absorbed: 0.0,
            just_absorbed: false,
            enabled: true,
        }
    }

    /// Apply flat absorption to `incoming`. Returns the damage that passes
    /// through (`incoming - absorption`, floored at 0.0). Fires
    /// `just_absorbed` and increments `hits_absorbed` when any absorption
    /// occurs. Returns `incoming` unmodified when disabled.
    pub fn absorb(&mut self, incoming: f32) -> f32 {
        if !self.enabled {
            return incoming;
        }
        let absorbed = incoming.min(self.absorption_per_hit);
        self.last_absorbed = absorbed;
        if absorbed > 0.0 {
            self.just_absorbed = true;
            self.hits_absorbed += 1;
        }
        (incoming - absorbed).max(0.0)
    }

    /// Advance one frame — clears `just_absorbed`. No-op when disabled.
    pub fn tick(&mut self, _dt: f32) {
        if !self.enabled {
            return;
        }
        self.just_absorbed = false;
    }

    /// `true` when the vest can absorb damage (`absorption_per_hit > 0.0`)
    /// and the component is enabled.
    pub fn is_absorbing(&self) -> bool {
        self.absorption_per_hit > 0.0 && self.enabled
    }

    /// Pure query: damage that would pass through after absorption.
    /// Does **not** mutate `hits_absorbed`, `last_absorbed`, or `just_absorbed`.
    /// Returns `(base - absorption_per_hit).max(0.0)` when enabled; `base`
    /// otherwise.
    pub fn effective_incoming(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        (base - self.absorption_per_hit).max(0.0)
    }
}

impl Default for Vest {
    fn default() -> Self {
        Self::new(5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_zero_hits() {
        let v = Vest::new(5.0);
        assert_eq!(v.hits_absorbed, 0);
        assert_eq!(v.last_absorbed, 0.0);
        assert!(!v.just_absorbed);
    }

    #[test]
    fn absorb_subtracts_from_incoming() {
        let mut v = Vest::new(5.0);
        let result = v.absorb(10.0);
        assert!((result - 5.0).abs() < 1e-5);
    }

    #[test]
    fn absorb_floors_at_zero_when_absorption_exceeds_incoming() {
        let mut v = Vest::new(10.0);
        let result = v.absorb(3.0);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn absorb_records_last_absorbed() {
        let mut v = Vest::new(5.0);
        v.absorb(3.0); // absorbed = 3 (capped to incoming)
        assert!((v.last_absorbed - 3.0).abs() < 1e-5);
    }

    #[test]
    fn absorb_records_full_absorption_when_incoming_exceeds() {
        let mut v = Vest::new(5.0);
        v.absorb(12.0);
        assert!((v.last_absorbed - 5.0).abs() < 1e-5);
    }

    #[test]
    fn absorb_fires_just_absorbed() {
        let mut v = Vest::new(5.0);
        v.absorb(10.0);
        assert!(v.just_absorbed);
    }

    #[test]
    fn absorb_increments_hits_absorbed() {
        let mut v = Vest::new(5.0);
        v.absorb(10.0);
        v.tick(0.016);
        v.absorb(10.0);
        assert_eq!(v.hits_absorbed, 2);
    }

    #[test]
    fn absorb_no_just_absorbed_when_incoming_zero() {
        let mut v = Vest::new(5.0);
        v.absorb(0.0);
        assert!(!v.just_absorbed);
    }

    #[test]
    fn absorb_no_just_absorbed_when_absorption_zero() {
        let mut v = Vest::new(0.0);
        v.absorb(10.0);
        assert!(!v.just_absorbed);
    }

    #[test]
    fn absorb_no_increment_when_no_absorption() {
        let mut v = Vest::new(0.0);
        v.absorb(10.0);
        assert_eq!(v.hits_absorbed, 0);
    }

    #[test]
    fn absorb_returns_incoming_when_disabled() {
        let mut v = Vest::new(5.0);
        v.enabled = false;
        let result = v.absorb(10.0);
        assert!((result - 10.0).abs() < 1e-5);
        assert!(!v.just_absorbed);
        assert_eq!(v.hits_absorbed, 0);
    }

    #[test]
    fn absorb_returns_incoming_when_disabled_negative_check() {
        let mut v = Vest::new(5.0);
        v.enabled = false;
        let result = v.absorb(3.0);
        assert!((result - 3.0).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_just_absorbed() {
        let mut v = Vest::new(5.0);
        v.absorb(10.0);
        assert!(v.just_absorbed);
        v.tick(0.016);
        assert!(!v.just_absorbed);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut v = Vest::new(5.0);
        v.just_absorbed = true; // manually set
        v.enabled = false;
        v.tick(0.016);
        assert!(v.just_absorbed); // not cleared because disabled
    }

    #[test]
    fn is_absorbing_true_when_positive_absorption_and_enabled() {
        let v = Vest::new(5.0);
        assert!(v.is_absorbing());
    }

    #[test]
    fn is_absorbing_false_when_absorption_zero() {
        let v = Vest::new(0.0);
        assert!(!v.is_absorbing());
    }

    #[test]
    fn is_absorbing_false_when_disabled() {
        let mut v = Vest::new(5.0);
        v.enabled = false;
        assert!(!v.is_absorbing());
    }

    #[test]
    fn effective_incoming_subtracts_absorption() {
        let v = Vest::new(5.0);
        assert!((v.effective_incoming(12.0) - 7.0).abs() < 1e-5);
    }

    #[test]
    fn effective_incoming_floors_at_zero() {
        let v = Vest::new(10.0);
        assert_eq!(v.effective_incoming(3.0), 0.0);
    }

    #[test]
    fn effective_incoming_base_when_disabled() {
        let mut v = Vest::new(5.0);
        v.enabled = false;
        assert!((v.effective_incoming(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn effective_incoming_does_not_mutate_hits_absorbed() {
        let v = Vest::new(5.0);
        v.effective_incoming(10.0);
        assert_eq!(v.hits_absorbed, 0);
    }

    #[test]
    fn effective_incoming_does_not_mutate_just_absorbed() {
        let v = Vest::new(5.0);
        v.effective_incoming(10.0);
        assert!(!v.just_absorbed);
    }

    #[test]
    fn effective_incoming_does_not_mutate_last_absorbed() {
        let v = Vest::new(5.0);
        v.effective_incoming(10.0);
        assert_eq!(v.last_absorbed, 0.0);
    }

    #[test]
    fn absorption_per_hit_clamped_to_zero() {
        let v = Vest::new(-3.0);
        assert_eq!(v.absorption_per_hit, 0.0);
    }

    #[test]
    fn multiple_hits_accumulate_count() {
        let mut v = Vest::new(5.0);
        for _ in 0..5 {
            v.absorb(10.0);
            v.tick(0.016);
        }
        assert_eq!(v.hits_absorbed, 5);
    }

    #[test]
    fn zero_incoming_no_absorption_no_flag() {
        let mut v = Vest::new(5.0);
        let result = v.absorb(0.0);
        assert_eq!(result, 0.0);
        assert!(!v.just_absorbed);
        assert_eq!(v.last_absorbed, 0.0);
    }

    #[test]
    fn absorb_exact_match_leaves_zero_through() {
        let mut v = Vest::new(5.0);
        let result = v.absorb(5.0);
        assert_eq!(result, 0.0);
        assert!((v.last_absorbed - 5.0).abs() < 1e-5);
    }

    #[test]
    fn just_absorbed_persists_until_tick() {
        let mut v = Vest::new(5.0);
        v.absorb(10.0);
        assert!(v.just_absorbed); // still set before tick
        v.absorb(10.0); // second hit before tick
        assert!(v.just_absorbed);
        v.tick(0.016); // now cleared
        assert!(!v.just_absorbed);
    }
}
