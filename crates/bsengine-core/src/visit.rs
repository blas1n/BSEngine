use bevy_ecs::prelude::Component;

/// Encounter-proximity accumulation tracker named after visit, the
/// verb and noun meaning to go to see a person, place, or thing,
/// especially for a short time and for a specific purpose — from the
/// Latin visitare, frequentative of visere (to go to see), itself a
/// frequentative of videre (to see). The word carries in its etymology
/// the idea of directed, purposeful movement toward an object of
/// interest: one does not visit by accident but by intention, even if
/// the intention is merely social. In English the word bifurcated early
/// into two registers: the formal visit of an official inspection or
/// diplomatic call, and the informal visit of neighbours crossing a
/// property boundary for tea. Both senses share the core structure of
/// a temporary, bounded contact between a visitor and a visited — the
/// relationship is asymmetric, the direction of travel is specified,
/// and the visit ends when the visitor withdraws. In ecology, the
/// visit as a unit of analysis appears in pollination studies, where
/// a flower visit is the discrete event of a pollinator landing on
/// and interacting with a flower — measurable, countable, consequential
/// for both parties. In epidemiology, the contact is the unit from
/// which transmission networks are built: each visit between two
/// individuals is an edge in the graph along which pathogens, or
/// information, or influence can travel. In game mechanics, a visit
/// mechanic tracks the cumulative proximity exposure of one entity to
/// another — the slow build of familiarity, the accumulation of
/// encounter time, the saturation of contact before a threshold is
/// crossed and the relationship changes. `exposure` builds via
/// `approach(amount)` and accumulates passively at `presence_rate`
/// per second in `tick(dt)` or dissipates via `withdraw(amount)`.
///
/// Models encounter-proximity fill levels, familiarity-saturation
/// bars, contact-accumulation trackers, pollinator-visit gauges,
/// diplomatic-approach fill levels, social-exposure saturation
/// indicators, stealth-proximity accumulation bars, detection-range
/// meters, aggro-range fill levels, or any mechanic where one entity
/// slowly accumulates exposure to another as a function of proximity
/// and time — the creeping fill of a detection bar, the gradual build
/// of familiarity, the silent accumulation of contact until a threshold
/// is crossed and the relationship irrevocably changes.
///
/// `approach(amount)` adds exposure; fires `just_visited` when first
/// reaching `max_exposure`. No-op when disabled.
///
/// `withdraw(amount)` reduces exposure immediately; fires
/// `just_departed` when reaching 0. No-op when disabled or already
/// departed.
///
/// `tick(dt)` clears both flags, then increases exposure by
/// `presence_rate * dt` (capped at `max_exposure`). Fires
/// `just_visited` when first reaching max. No-op when disabled or
/// rate is 0.
///
/// `is_visited()` returns `exposure >= max_exposure && enabled`.
///
/// `is_departed()` returns `exposure == 0.0` (not gated by `enabled`).
///
/// `exposure_fraction()` returns
/// `(exposure / max_exposure).clamp(0, 1)`.
///
/// `effective_presence(scale)` returns `scale * exposure_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — accumulates at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Visit {
    pub exposure: f32,
    pub max_exposure: f32,
    pub presence_rate: f32,
    pub just_visited: bool,
    pub just_departed: bool,
    pub enabled: bool,
}

impl Visit {
    pub fn new(max_exposure: f32, presence_rate: f32) -> Self {
        Self {
            exposure: 0.0,
            max_exposure: max_exposure.max(0.1),
            presence_rate: presence_rate.max(0.0),
            just_visited: false,
            just_departed: false,
            enabled: true,
        }
    }

    /// Add exposure; fires `just_visited` when first reaching max.
    /// No-op when disabled.
    pub fn approach(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.exposure < self.max_exposure;
        self.exposure = (self.exposure + amount).min(self.max_exposure);
        if was_below && self.exposure >= self.max_exposure {
            self.just_visited = true;
        }
    }

    /// Reduce exposure; fires `just_departed` when reaching 0.
    /// No-op when disabled or already departed.
    pub fn withdraw(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.exposure <= 0.0 {
            return;
        }
        self.exposure = (self.exposure - amount).max(0.0);
        if self.exposure <= 0.0 {
            self.just_departed = true;
        }
    }

    /// Clear flags, then increase exposure by `presence_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_visited = false;
        self.just_departed = false;
        if self.enabled && self.presence_rate > 0.0 && self.exposure < self.max_exposure {
            let was_below = self.exposure < self.max_exposure;
            self.exposure = (self.exposure + self.presence_rate * dt).min(self.max_exposure);
            if was_below && self.exposure >= self.max_exposure {
                self.just_visited = true;
            }
        }
    }

    /// `true` when exposure is at maximum and component is enabled.
    pub fn is_visited(&self) -> bool {
        self.exposure >= self.max_exposure && self.enabled
    }

    /// `true` when exposure is 0 (not gated by `enabled`).
    pub fn is_departed(&self) -> bool {
        self.exposure == 0.0
    }

    /// Fraction of maximum exposure [0.0, 1.0].
    pub fn exposure_fraction(&self) -> f32 {
        (self.exposure / self.max_exposure).clamp(0.0, 1.0)
    }

    /// Returns `scale * exposure_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_presence(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.exposure_fraction()
    }
}

impl Default for Visit {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v() -> Visit {
        Visit::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_departed() {
        let v = v();
        assert_eq!(v.exposure, 0.0);
        assert!(v.is_departed());
        assert!(!v.is_visited());
    }

    #[test]
    fn new_clamps_max_exposure() {
        let v = Visit::new(-5.0, 1.5);
        assert!((v.max_exposure - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_presence_rate() {
        let v = Visit::new(100.0, -1.5);
        assert_eq!(v.presence_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let v = Visit::default();
        assert!((v.max_exposure - 100.0).abs() < 1e-5);
        assert!((v.presence_rate - 1.5).abs() < 1e-5);
    }

    // --- approach ---

    #[test]
    fn approach_adds_exposure() {
        let mut v = v();
        v.approach(40.0);
        assert!((v.exposure - 40.0).abs() < 1e-3);
    }

    #[test]
    fn approach_clamps_at_max() {
        let mut v = v();
        v.approach(200.0);
        assert!((v.exposure - 100.0).abs() < 1e-3);
    }

    #[test]
    fn approach_fires_just_visited_at_max() {
        let mut v = v();
        v.approach(100.0);
        assert!(v.just_visited);
        assert!(v.is_visited());
    }

    #[test]
    fn approach_no_just_visited_when_already_at_max() {
        let mut v = v();
        v.exposure = 100.0;
        v.approach(10.0);
        assert!(!v.just_visited);
    }

    #[test]
    fn approach_no_op_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.approach(50.0);
        assert_eq!(v.exposure, 0.0);
    }

    #[test]
    fn approach_no_op_when_amount_zero() {
        let mut v = v();
        v.approach(0.0);
        assert_eq!(v.exposure, 0.0);
    }

    // --- withdraw ---

    #[test]
    fn withdraw_reduces_exposure() {
        let mut v = v();
        v.exposure = 60.0;
        v.withdraw(20.0);
        assert!((v.exposure - 40.0).abs() < 1e-3);
    }

    #[test]
    fn withdraw_clamps_at_zero() {
        let mut v = v();
        v.exposure = 30.0;
        v.withdraw(200.0);
        assert_eq!(v.exposure, 0.0);
    }

    #[test]
    fn withdraw_fires_just_departed_at_zero() {
        let mut v = v();
        v.exposure = 30.0;
        v.withdraw(30.0);
        assert!(v.just_departed);
    }

    #[test]
    fn withdraw_no_op_when_already_departed() {
        let mut v = v();
        v.withdraw(10.0);
        assert!(!v.just_departed);
    }

    #[test]
    fn withdraw_no_op_when_disabled() {
        let mut v = v();
        v.exposure = 50.0;
        v.enabled = false;
        v.withdraw(50.0);
        assert!((v.exposure - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_exposure() {
        let mut v = v(); // rate=1.5
        v.tick(4.0); // 0 + 1.5*4 = 6
        assert!((v.exposure - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_visited_on_exposure_to_max() {
        let mut v = Visit::new(100.0, 200.0);
        v.exposure = 95.0;
        v.tick(1.0);
        assert!(v.just_visited);
        assert!(v.is_visited());
    }

    #[test]
    fn tick_no_build_when_already_visited() {
        let mut v = v();
        v.exposure = 100.0;
        v.tick(1.0);
        assert!(!v.just_visited);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut v = Visit::new(100.0, 0.0);
        v.tick(100.0);
        assert_eq!(v.exposure, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.tick(1.0);
        assert_eq!(v.exposure, 0.0);
    }

    #[test]
    fn tick_clears_just_visited() {
        let mut v = Visit::new(100.0, 200.0);
        v.exposure = 95.0;
        v.tick(1.0);
        v.tick(0.016);
        assert!(!v.just_visited);
    }

    #[test]
    fn tick_clears_just_departed() {
        let mut v = v();
        v.exposure = 10.0;
        v.withdraw(10.0);
        v.tick(0.016);
        assert!(!v.just_departed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut v = v(); // rate=1.5
        v.tick(6.0); // 1.5*6 = 9
        assert!((v.exposure - 9.0).abs() < 1e-3);
    }

    // --- is_visited / is_departed ---

    #[test]
    fn is_visited_false_when_disabled() {
        let mut v = v();
        v.exposure = 100.0;
        v.enabled = false;
        assert!(!v.is_visited());
    }

    #[test]
    fn is_departed_not_gated_by_enabled() {
        let mut v = v();
        v.enabled = false;
        assert!(v.is_departed());
    }

    // --- exposure_fraction / effective_presence ---

    #[test]
    fn exposure_fraction_zero_when_departed() {
        assert_eq!(v().exposure_fraction(), 0.0);
    }

    #[test]
    fn exposure_fraction_half_at_midpoint() {
        let mut v = v();
        v.exposure = 50.0;
        assert!((v.exposure_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_presence_zero_when_departed() {
        assert_eq!(v().effective_presence(100.0), 0.0);
    }

    #[test]
    fn effective_presence_scales_with_exposure() {
        let mut v = v();
        v.exposure = 75.0;
        assert!((v.effective_presence(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_presence_zero_when_disabled() {
        let mut v = v();
        v.exposure = 50.0;
        v.enabled = false;
        assert_eq!(v.effective_presence(100.0), 0.0);
    }
}
