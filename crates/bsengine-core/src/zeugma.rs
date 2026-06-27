use bevy_ecs::prelude::Component;

/// Linkage-tension tracker. `tension` builds via `yoke(amount)` and
/// tightens passively at `tighten_rate` per second in `tick(dt)` or
/// snaps immediately via `sever(amount)`.
///
/// Models chain-link strength gauges, bond-tension meters in tethering
/// systems, rope-tension trackers for grappling mechanics, team-synergy
/// bars, dual-wielding-coordination indicators, or any mechanic where
/// two elements must stay linked and the connection strengthens through
/// sustained coupling but instantly releases when force is applied.
///
/// `yoke(amount)` adds tension; fires `just_yoked` when first reaching
/// `max_tension`. No-op when disabled.
///
/// `sever(amount)` reduces tension immediately; fires `just_severed`
/// when reaching 0. No-op when disabled or already severed.
///
/// `tick(dt)` clears both flags, then increases tension by
/// `tighten_rate * dt` (capped at `max_tension`). Fires `just_yoked`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_yoked()` returns `tension >= max_tension && enabled`.
///
/// `is_severed()` returns `tension == 0.0` (not gated by `enabled`).
///
/// `tension_fraction()` returns `(tension / max_tension).clamp(0, 1)`.
///
/// `effective_coupling(scale)` returns `scale * tension_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 6.0)` — tightens at 6 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zeugma {
    pub tension: f32,
    pub max_tension: f32,
    pub tighten_rate: f32,
    pub just_yoked: bool,
    pub just_severed: bool,
    pub enabled: bool,
}

impl Zeugma {
    pub fn new(max_tension: f32, tighten_rate: f32) -> Self {
        Self {
            tension: 0.0,
            max_tension: max_tension.max(0.1),
            tighten_rate: tighten_rate.max(0.0),
            just_yoked: false,
            just_severed: false,
            enabled: true,
        }
    }

    /// Add tension; fires `just_yoked` when first reaching max.
    /// No-op when disabled.
    pub fn yoke(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.tension < self.max_tension;
        self.tension = (self.tension + amount).min(self.max_tension);
        if was_below && self.tension >= self.max_tension {
            self.just_yoked = true;
        }
    }

    /// Reduce tension; fires `just_severed` when reaching 0.
    /// No-op when disabled or already severed.
    pub fn sever(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.tension <= 0.0 {
            return;
        }
        self.tension = (self.tension - amount).max(0.0);
        if self.tension <= 0.0 {
            self.just_severed = true;
        }
    }

    /// Clear flags, then increase tension by `tighten_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_yoked = false;
        self.just_severed = false;
        if self.enabled && self.tighten_rate > 0.0 && self.tension < self.max_tension {
            let was_below = self.tension < self.max_tension;
            self.tension = (self.tension + self.tighten_rate * dt).min(self.max_tension);
            if was_below && self.tension >= self.max_tension {
                self.just_yoked = true;
            }
        }
    }

    /// `true` when tension is at maximum and component is enabled.
    pub fn is_yoked(&self) -> bool {
        self.tension >= self.max_tension && self.enabled
    }

    /// `true` when tension is 0 (not gated by `enabled`).
    pub fn is_severed(&self) -> bool {
        self.tension == 0.0
    }

    /// Fraction of maximum tension [0.0, 1.0].
    pub fn tension_fraction(&self) -> f32 {
        (self.tension / self.max_tension).clamp(0.0, 1.0)
    }

    /// Returns `scale * tension_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_coupling(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.tension_fraction()
    }
}

impl Default for Zeugma {
    fn default() -> Self {
        Self::new(100.0, 6.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zeugma {
        Zeugma::new(100.0, 6.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_severed() {
        let z = z();
        assert_eq!(z.tension, 0.0);
        assert!(z.is_severed());
        assert!(!z.is_yoked());
    }

    #[test]
    fn new_clamps_max_tension() {
        let z = Zeugma::new(-5.0, 6.0);
        assert!((z.max_tension - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_tighten_rate() {
        let z = Zeugma::new(100.0, -3.0);
        assert_eq!(z.tighten_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zeugma::default();
        assert!((z.max_tension - 100.0).abs() < 1e-5);
        assert!((z.tighten_rate - 6.0).abs() < 1e-5);
    }

    // --- yoke ---

    #[test]
    fn yoke_adds_tension() {
        let mut z = z();
        z.yoke(40.0);
        assert!((z.tension - 40.0).abs() < 1e-3);
    }

    #[test]
    fn yoke_clamps_at_max() {
        let mut z = z();
        z.yoke(200.0);
        assert!((z.tension - 100.0).abs() < 1e-3);
    }

    #[test]
    fn yoke_fires_just_yoked_at_max() {
        let mut z = z();
        z.yoke(100.0);
        assert!(z.just_yoked);
        assert!(z.is_yoked());
    }

    #[test]
    fn yoke_no_just_yoked_when_already_at_max() {
        let mut z = z();
        z.tension = 100.0;
        z.yoke(10.0);
        assert!(!z.just_yoked);
    }

    #[test]
    fn yoke_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.yoke(50.0);
        assert_eq!(z.tension, 0.0);
    }

    #[test]
    fn yoke_no_op_when_amount_zero() {
        let mut z = z();
        z.yoke(0.0);
        assert_eq!(z.tension, 0.0);
    }

    // --- sever ---

    #[test]
    fn sever_reduces_tension() {
        let mut z = z();
        z.tension = 60.0;
        z.sever(20.0);
        assert!((z.tension - 40.0).abs() < 1e-3);
    }

    #[test]
    fn sever_clamps_at_zero() {
        let mut z = z();
        z.tension = 30.0;
        z.sever(200.0);
        assert_eq!(z.tension, 0.0);
    }

    #[test]
    fn sever_fires_just_severed_at_zero() {
        let mut z = z();
        z.tension = 30.0;
        z.sever(30.0);
        assert!(z.just_severed);
    }

    #[test]
    fn sever_no_op_when_already_severed() {
        let mut z = z();
        z.sever(10.0);
        assert!(!z.just_severed);
    }

    #[test]
    fn sever_no_op_when_disabled() {
        let mut z = z();
        z.tension = 50.0;
        z.enabled = false;
        z.sever(50.0);
        assert!((z.tension - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_tightens_tension() {
        let mut z = z(); // rate=6
        z.tick(1.0); // 0 + 6 = 6
        assert!((z.tension - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_yoked_on_tighten_to_max() {
        let mut z = Zeugma::new(100.0, 200.0);
        z.tension = 95.0;
        z.tick(1.0);
        assert!(z.just_yoked);
        assert!(z.is_yoked());
    }

    #[test]
    fn tick_no_tighten_when_already_yoked() {
        let mut z = z();
        z.tension = 100.0;
        z.tick(1.0);
        assert!(!z.just_yoked);
    }

    #[test]
    fn tick_no_tighten_when_rate_zero() {
        let mut z = Zeugma::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.tension, 0.0);
    }

    #[test]
    fn tick_no_tighten_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.tension, 0.0);
    }

    #[test]
    fn tick_clears_just_yoked() {
        let mut z = Zeugma::new(100.0, 200.0);
        z.tension = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_yoked);
    }

    #[test]
    fn tick_clears_just_severed() {
        let mut z = z();
        z.tension = 10.0;
        z.sever(10.0);
        z.tick(0.016);
        assert!(!z.just_severed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=6
        z.tick(5.0); // 6*5 = 30
        assert!((z.tension - 30.0).abs() < 1e-3);
    }

    // --- is_yoked / is_severed ---

    #[test]
    fn is_yoked_false_when_disabled() {
        let mut z = z();
        z.tension = 100.0;
        z.enabled = false;
        assert!(!z.is_yoked());
    }

    #[test]
    fn is_severed_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_severed());
    }

    // --- tension_fraction / effective_coupling ---

    #[test]
    fn tension_fraction_zero_when_severed() {
        assert_eq!(z().tension_fraction(), 0.0);
    }

    #[test]
    fn tension_fraction_half_at_midpoint() {
        let mut z = z();
        z.tension = 50.0;
        assert!((z.tension_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_coupling_zero_when_severed() {
        assert_eq!(z().effective_coupling(100.0), 0.0);
    }

    #[test]
    fn effective_coupling_scales_with_tension() {
        let mut z = z();
        z.tension = 75.0;
        assert!((z.effective_coupling(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_coupling_zero_when_disabled() {
        let mut z = z();
        z.tension = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_coupling(100.0), 0.0);
    }
}
