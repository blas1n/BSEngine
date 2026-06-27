use bevy_ecs::prelude::Component;

/// Territory-control tracker. `influence` builds via `claim(amount)` and
/// expands passively at `spread_rate` per second in `tick(dt)` or is
/// contested immediately via `contest(amount)`.
///
/// Models urban-planning zone fill levels, area-of-control saturation
/// bars, territory-claim progress trackers, administrative-district
/// coverage gauges, land-use enforcement intensity meters, rezoning
/// momentum indicators, sphere-of-influence expansion bars, tactical
/// territory saturation fill levels, colony-settlement control trackers,
/// or any mechanic where patient accumulation of administrative reach
/// gradually saturates a region with the claimant's authority until
/// a rival contests the boundary and the whole careful project must be
/// rebuilt from whatever fractured remnant political reality permits.
///
/// `claim(amount)` adds influence; fires `just_dominant` when first
/// reaching `max_influence`. No-op when disabled.
///
/// `contest(amount)` reduces influence immediately; fires `just_neutral`
/// when reaching 0. No-op when disabled or already neutral.
///
/// `tick(dt)` clears both flags, then increases influence by
/// `spread_rate * dt` (capped at `max_influence`). Fires `just_dominant`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_dominant()` returns `influence >= max_influence && enabled`.
///
/// `is_neutral()` returns `influence == 0.0` (not gated by `enabled`).
///
/// `influence_fraction()` returns `(influence / max_influence).clamp(0, 1)`.
///
/// `effective_control(scale)` returns `scale * influence_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 2.0)` — spreads at 2 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoning {
    pub influence: f32,
    pub max_influence: f32,
    pub spread_rate: f32,
    pub just_dominant: bool,
    pub just_neutral: bool,
    pub enabled: bool,
}

impl Zoning {
    pub fn new(max_influence: f32, spread_rate: f32) -> Self {
        Self {
            influence: 0.0,
            max_influence: max_influence.max(0.1),
            spread_rate: spread_rate.max(0.0),
            just_dominant: false,
            just_neutral: false,
            enabled: true,
        }
    }

    /// Add influence; fires `just_dominant` when first reaching max.
    /// No-op when disabled.
    pub fn claim(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.influence < self.max_influence;
        self.influence = (self.influence + amount).min(self.max_influence);
        if was_below && self.influence >= self.max_influence {
            self.just_dominant = true;
        }
    }

    /// Reduce influence; fires `just_neutral` when reaching 0.
    /// No-op when disabled or already neutral.
    pub fn contest(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.influence <= 0.0 {
            return;
        }
        self.influence = (self.influence - amount).max(0.0);
        if self.influence <= 0.0 {
            self.just_neutral = true;
        }
    }

    /// Clear flags, then increase influence by `spread_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_dominant = false;
        self.just_neutral = false;
        if self.enabled && self.spread_rate > 0.0 && self.influence < self.max_influence {
            let was_below = self.influence < self.max_influence;
            self.influence = (self.influence + self.spread_rate * dt).min(self.max_influence);
            if was_below && self.influence >= self.max_influence {
                self.just_dominant = true;
            }
        }
    }

    /// `true` when influence is at maximum and component is enabled.
    pub fn is_dominant(&self) -> bool {
        self.influence >= self.max_influence && self.enabled
    }

    /// `true` when influence is 0 (not gated by `enabled`).
    pub fn is_neutral(&self) -> bool {
        self.influence == 0.0
    }

    /// Fraction of maximum influence [0.0, 1.0].
    pub fn influence_fraction(&self) -> f32 {
        (self.influence / self.max_influence).clamp(0.0, 1.0)
    }

    /// Returns `scale * influence_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_control(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.influence_fraction()
    }
}

impl Default for Zoning {
    fn default() -> Self {
        Self::new(100.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoning {
        Zoning::new(100.0, 2.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_neutral() {
        let z = z();
        assert_eq!(z.influence, 0.0);
        assert!(z.is_neutral());
        assert!(!z.is_dominant());
    }

    #[test]
    fn new_clamps_max_influence() {
        let z = Zoning::new(-5.0, 2.0);
        assert!((z.max_influence - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_spread_rate() {
        let z = Zoning::new(100.0, -2.0);
        assert_eq!(z.spread_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoning::default();
        assert!((z.max_influence - 100.0).abs() < 1e-5);
        assert!((z.spread_rate - 2.0).abs() < 1e-5);
    }

    // --- claim ---

    #[test]
    fn claim_adds_influence() {
        let mut z = z();
        z.claim(40.0);
        assert!((z.influence - 40.0).abs() < 1e-3);
    }

    #[test]
    fn claim_clamps_at_max() {
        let mut z = z();
        z.claim(200.0);
        assert!((z.influence - 100.0).abs() < 1e-3);
    }

    #[test]
    fn claim_fires_just_dominant_at_max() {
        let mut z = z();
        z.claim(100.0);
        assert!(z.just_dominant);
        assert!(z.is_dominant());
    }

    #[test]
    fn claim_no_just_dominant_when_already_at_max() {
        let mut z = z();
        z.influence = 100.0;
        z.claim(10.0);
        assert!(!z.just_dominant);
    }

    #[test]
    fn claim_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.claim(50.0);
        assert_eq!(z.influence, 0.0);
    }

    #[test]
    fn claim_no_op_when_amount_zero() {
        let mut z = z();
        z.claim(0.0);
        assert_eq!(z.influence, 0.0);
    }

    // --- contest ---

    #[test]
    fn contest_reduces_influence() {
        let mut z = z();
        z.influence = 60.0;
        z.contest(20.0);
        assert!((z.influence - 40.0).abs() < 1e-3);
    }

    #[test]
    fn contest_clamps_at_zero() {
        let mut z = z();
        z.influence = 30.0;
        z.contest(200.0);
        assert_eq!(z.influence, 0.0);
    }

    #[test]
    fn contest_fires_just_neutral_at_zero() {
        let mut z = z();
        z.influence = 30.0;
        z.contest(30.0);
        assert!(z.just_neutral);
    }

    #[test]
    fn contest_no_op_when_already_neutral() {
        let mut z = z();
        z.contest(10.0);
        assert!(!z.just_neutral);
    }

    #[test]
    fn contest_no_op_when_disabled() {
        let mut z = z();
        z.influence = 50.0;
        z.enabled = false;
        z.contest(50.0);
        assert!((z.influence - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_spreads_influence() {
        let mut z = z(); // rate=2
        z.tick(3.0); // 0 + 2*3 = 6
        assert!((z.influence - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_dominant_on_spread_to_max() {
        let mut z = Zoning::new(100.0, 200.0);
        z.influence = 95.0;
        z.tick(1.0);
        assert!(z.just_dominant);
        assert!(z.is_dominant());
    }

    #[test]
    fn tick_no_spread_when_already_dominant() {
        let mut z = z();
        z.influence = 100.0;
        z.tick(1.0);
        assert!(!z.just_dominant);
    }

    #[test]
    fn tick_no_spread_when_rate_zero() {
        let mut z = Zoning::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.influence, 0.0);
    }

    #[test]
    fn tick_no_spread_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.influence, 0.0);
    }

    #[test]
    fn tick_clears_just_dominant() {
        let mut z = Zoning::new(100.0, 200.0);
        z.influence = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_dominant);
    }

    #[test]
    fn tick_clears_just_neutral() {
        let mut z = z();
        z.influence = 10.0;
        z.contest(10.0);
        z.tick(0.016);
        assert!(!z.just_neutral);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=2
        z.tick(5.0); // 2*5 = 10
        assert!((z.influence - 10.0).abs() < 1e-3);
    }

    // --- is_dominant / is_neutral ---

    #[test]
    fn is_dominant_false_when_disabled() {
        let mut z = z();
        z.influence = 100.0;
        z.enabled = false;
        assert!(!z.is_dominant());
    }

    #[test]
    fn is_neutral_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_neutral());
    }

    // --- influence_fraction / effective_control ---

    #[test]
    fn influence_fraction_zero_when_neutral() {
        assert_eq!(z().influence_fraction(), 0.0);
    }

    #[test]
    fn influence_fraction_half_at_midpoint() {
        let mut z = z();
        z.influence = 50.0;
        assert!((z.influence_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_control_zero_when_neutral() {
        assert_eq!(z().effective_control(100.0), 0.0);
    }

    #[test]
    fn effective_control_scales_with_influence() {
        let mut z = z();
        z.influence = 75.0;
        assert!((z.effective_control(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_control_zero_when_disabled() {
        let mut z = z();
        z.influence = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_control(100.0), 0.0);
    }
}
