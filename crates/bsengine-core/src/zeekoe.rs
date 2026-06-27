use bevy_ecs::prelude::Component;

/// Wallow-territory tracker. `territory` builds via `claim(amount)` and
/// expands passively at `wallow_rate` per second in `tick(dt)` or is
/// contested immediately via `contest(amount)`.
///
/// Models hippopotamus wallow-pool ownership bars, wetland-territory
/// accumulation trackers, riparian-zone dominance fill levels,
/// waterhole-claim intensity gauges, river-access control meters,
/// mud-pool saturation indicators, amphibious-range control trackers,
/// or any mechanic where a massively territorial creature slowly
/// saturates every waterhole in sight before a rival challenges its claim.
///
/// `claim(amount)` adds territory; fires `just_dominant` when first
/// reaching `max_territory`. No-op when disabled.
///
/// `contest(amount)` reduces territory immediately; fires `just_displaced`
/// when reaching 0. No-op when disabled or already displaced.
///
/// `tick(dt)` clears both flags, then increases territory by
/// `wallow_rate * dt` (capped at `max_territory`). Fires `just_dominant`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_dominant()` returns `territory >= max_territory && enabled`.
///
/// `is_displaced()` returns `territory == 0.0` (not gated by `enabled`).
///
/// `territory_fraction()` returns `(territory / max_territory).clamp(0, 1)`.
///
/// `effective_presence(scale)` returns `scale * territory_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — wallows at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zeekoe {
    pub territory: f32,
    pub max_territory: f32,
    pub wallow_rate: f32,
    pub just_dominant: bool,
    pub just_displaced: bool,
    pub enabled: bool,
}

impl Zeekoe {
    pub fn new(max_territory: f32, wallow_rate: f32) -> Self {
        Self {
            territory: 0.0,
            max_territory: max_territory.max(0.1),
            wallow_rate: wallow_rate.max(0.0),
            just_dominant: false,
            just_displaced: false,
            enabled: true,
        }
    }

    /// Add territory; fires `just_dominant` when first reaching max.
    /// No-op when disabled.
    pub fn claim(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.territory < self.max_territory;
        self.territory = (self.territory + amount).min(self.max_territory);
        if was_below && self.territory >= self.max_territory {
            self.just_dominant = true;
        }
    }

    /// Reduce territory; fires `just_displaced` when reaching 0.
    /// No-op when disabled or already displaced.
    pub fn contest(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.territory <= 0.0 {
            return;
        }
        self.territory = (self.territory - amount).max(0.0);
        if self.territory <= 0.0 {
            self.just_displaced = true;
        }
    }

    /// Clear flags, then increase territory by `wallow_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_dominant = false;
        self.just_displaced = false;
        if self.enabled && self.wallow_rate > 0.0 && self.territory < self.max_territory {
            let was_below = self.territory < self.max_territory;
            self.territory = (self.territory + self.wallow_rate * dt).min(self.max_territory);
            if was_below && self.territory >= self.max_territory {
                self.just_dominant = true;
            }
        }
    }

    /// `true` when territory is at maximum and component is enabled.
    pub fn is_dominant(&self) -> bool {
        self.territory >= self.max_territory && self.enabled
    }

    /// `true` when territory is 0 (not gated by `enabled`).
    pub fn is_displaced(&self) -> bool {
        self.territory == 0.0
    }

    /// Fraction of maximum territory [0.0, 1.0].
    pub fn territory_fraction(&self) -> f32 {
        (self.territory / self.max_territory).clamp(0.0, 1.0)
    }

    /// Returns `scale * territory_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_presence(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.territory_fraction()
    }
}

impl Default for Zeekoe {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zeekoe {
        Zeekoe::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_displaced() {
        let z = z();
        assert_eq!(z.territory, 0.0);
        assert!(z.is_displaced());
        assert!(!z.is_dominant());
    }

    #[test]
    fn new_clamps_max_territory() {
        let z = Zeekoe::new(-5.0, 1.5);
        assert!((z.max_territory - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_wallow_rate() {
        let z = Zeekoe::new(100.0, -3.0);
        assert_eq!(z.wallow_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zeekoe::default();
        assert!((z.max_territory - 100.0).abs() < 1e-5);
        assert!((z.wallow_rate - 1.5).abs() < 1e-5);
    }

    // --- claim ---

    #[test]
    fn claim_adds_territory() {
        let mut z = z();
        z.claim(40.0);
        assert!((z.territory - 40.0).abs() < 1e-3);
    }

    #[test]
    fn claim_clamps_at_max() {
        let mut z = z();
        z.claim(200.0);
        assert!((z.territory - 100.0).abs() < 1e-3);
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
        z.territory = 100.0;
        z.claim(10.0);
        assert!(!z.just_dominant);
    }

    #[test]
    fn claim_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.claim(50.0);
        assert_eq!(z.territory, 0.0);
    }

    #[test]
    fn claim_no_op_when_amount_zero() {
        let mut z = z();
        z.claim(0.0);
        assert_eq!(z.territory, 0.0);
    }

    // --- contest ---

    #[test]
    fn contest_reduces_territory() {
        let mut z = z();
        z.territory = 60.0;
        z.contest(20.0);
        assert!((z.territory - 40.0).abs() < 1e-3);
    }

    #[test]
    fn contest_clamps_at_zero() {
        let mut z = z();
        z.territory = 30.0;
        z.contest(200.0);
        assert_eq!(z.territory, 0.0);
    }

    #[test]
    fn contest_fires_just_displaced_at_zero() {
        let mut z = z();
        z.territory = 30.0;
        z.contest(30.0);
        assert!(z.just_displaced);
    }

    #[test]
    fn contest_no_op_when_already_displaced() {
        let mut z = z();
        z.contest(10.0);
        assert!(!z.just_displaced);
    }

    #[test]
    fn contest_no_op_when_disabled() {
        let mut z = z();
        z.territory = 50.0;
        z.enabled = false;
        z.contest(50.0);
        assert!((z.territory - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_expands_territory() {
        let mut z = z(); // rate=1.5
        z.tick(2.0); // 0 + 1.5*2 = 3
        assert!((z.territory - 3.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_dominant_on_claim_to_max() {
        let mut z = Zeekoe::new(100.0, 200.0);
        z.territory = 95.0;
        z.tick(1.0);
        assert!(z.just_dominant);
        assert!(z.is_dominant());
    }

    #[test]
    fn tick_no_expansion_when_already_dominant() {
        let mut z = z();
        z.territory = 100.0;
        z.tick(1.0);
        assert!(!z.just_dominant);
    }

    #[test]
    fn tick_no_expansion_when_rate_zero() {
        let mut z = Zeekoe::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.territory, 0.0);
    }

    #[test]
    fn tick_no_expansion_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.territory, 0.0);
    }

    #[test]
    fn tick_clears_just_dominant() {
        let mut z = Zeekoe::new(100.0, 200.0);
        z.territory = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_dominant);
    }

    #[test]
    fn tick_clears_just_displaced() {
        let mut z = z();
        z.territory = 10.0;
        z.contest(10.0);
        z.tick(0.016);
        assert!(!z.just_displaced);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 1.5*4 = 6
        assert!((z.territory - 6.0).abs() < 1e-3);
    }

    // --- is_dominant / is_displaced ---

    #[test]
    fn is_dominant_false_when_disabled() {
        let mut z = z();
        z.territory = 100.0;
        z.enabled = false;
        assert!(!z.is_dominant());
    }

    #[test]
    fn is_displaced_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_displaced());
    }

    // --- territory_fraction / effective_presence ---

    #[test]
    fn territory_fraction_zero_when_displaced() {
        assert_eq!(z().territory_fraction(), 0.0);
    }

    #[test]
    fn territory_fraction_half_at_midpoint() {
        let mut z = z();
        z.territory = 50.0;
        assert!((z.territory_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_presence_zero_when_displaced() {
        assert_eq!(z().effective_presence(100.0), 0.0);
    }

    #[test]
    fn effective_presence_scales_with_territory() {
        let mut z = z();
        z.territory = 75.0;
        assert!((z.effective_presence(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_presence_zero_when_disabled() {
        let mut z = z();
        z.territory = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_presence(100.0), 0.0);
    }
}
