use bevy_ecs::prelude::Component;

/// Tension-coil accumulation tracker named after wrung, the
/// past tense and past participle of wring — meaning to have
/// been squeezed, twisted, or compressed; the state of having
/// been wrung out; the condition of something from which
/// liquid has been extracted by twisting — from the Old
/// English wringan (to press, to squeeze), from the Proto-
/// Germanic wrangjaną, from the Proto-Indo-European root
/// wrengh- (to turn, to twist). Wrung is the completion
/// of wring: to have been wrung is to have passed through
/// the full cycle of compression and torsion, to have
/// arrived at the state in which no more can be extracted.
/// A cloth that has been wrung is one from which every
/// accessible drop of water has been removed by sustained
/// twisting pressure; hands that are wrung are hands whose
/// owner has been through sustained distress. The metaphorical
/// extension — wrung dry, wrung out, emotionally wrung —
/// preserves the sense of total depletion through applied
/// compression: to be wrung is to have had everything
/// compressible removed by sustained force. In game mechanics,
/// a wrung mechanic models the accumulation of torsional
/// tension — the coiling of a spring, the building of
/// rotational stress, the accumulation of twist-energy
/// that eventually reaches the threshold at which it
/// releases, uncoils, or triggers a torsion-dependent
/// effect. `tension` builds via `coil(amount)` and
/// accumulates passively at `twist_rate` per second in
/// `tick(dt)` or releases via `uncoil(amount)`.
///
/// Models torsion-tension fill levels, coil-saturation
/// bars, twist-accumulation trackers, spring-build gauges,
/// rotational-stress fill levels, wind-up saturation
/// indicators, torque-accumulation bars, coil-tension meters,
/// winding-completion fill levels, or any mechanic where
/// a device, creature, or system slowly accumulates the
/// rotational tension required to release a spring, trigger
/// a torque-based effect, or achieve the threshold of
/// maximum torsional compression.
///
/// `coil(amount)` adds tension; fires `just_wrung` when
/// first reaching `max_tension`. No-op when disabled.
///
/// `uncoil(amount)` reduces tension immediately; fires
/// `just_slack` when reaching 0. No-op when disabled or
/// already slack.
///
/// `tick(dt)` clears both flags, then increases tension
/// by `twist_rate * dt` (capped at `max_tension`). Fires
/// `just_wrung` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_wrung()` returns `tension >= max_tension && enabled`.
///
/// `is_slack()` returns `tension == 0.0` (not gated by
/// `enabled`).
///
/// `tension_fraction()` returns
/// `(tension / max_tension).clamp(0, 1)`.
///
/// `effective_torque(scale)` returns `scale * tension_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — coils at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wrung {
    pub tension: f32,
    pub max_tension: f32,
    pub twist_rate: f32,
    pub just_wrung: bool,
    pub just_slack: bool,
    pub enabled: bool,
}

impl Wrung {
    pub fn new(max_tension: f32, twist_rate: f32) -> Self {
        Self {
            tension: 0.0,
            max_tension: max_tension.max(0.1),
            twist_rate: twist_rate.max(0.0),
            just_wrung: false,
            just_slack: false,
            enabled: true,
        }
    }

    /// Add tension; fires `just_wrung` when first reaching max.
    /// No-op when disabled.
    pub fn coil(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.tension < self.max_tension;
        self.tension = (self.tension + amount).min(self.max_tension);
        if was_below && self.tension >= self.max_tension {
            self.just_wrung = true;
        }
    }

    /// Reduce tension; fires `just_slack` when reaching 0.
    /// No-op when disabled or already slack.
    pub fn uncoil(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.tension <= 0.0 {
            return;
        }
        self.tension = (self.tension - amount).max(0.0);
        if self.tension <= 0.0 {
            self.just_slack = true;
        }
    }

    /// Clear flags, then increase tension by `twist_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_wrung = false;
        self.just_slack = false;
        if self.enabled && self.twist_rate > 0.0 && self.tension < self.max_tension {
            let was_below = self.tension < self.max_tension;
            self.tension = (self.tension + self.twist_rate * dt).min(self.max_tension);
            if was_below && self.tension >= self.max_tension {
                self.just_wrung = true;
            }
        }
    }

    /// `true` when tension is at maximum and component is enabled.
    pub fn is_wrung(&self) -> bool {
        self.tension >= self.max_tension && self.enabled
    }

    /// `true` when tension is 0 (not gated by `enabled`).
    pub fn is_slack(&self) -> bool {
        self.tension == 0.0
    }

    /// Fraction of maximum tension [0.0, 1.0].
    pub fn tension_fraction(&self) -> f32 {
        (self.tension / self.max_tension).clamp(0.0, 1.0)
    }

    /// Returns `scale * tension_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_torque(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.tension_fraction()
    }
}

impl Default for Wrung {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wrung {
        Wrung::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_slack() {
        let w = w();
        assert_eq!(w.tension, 0.0);
        assert!(w.is_slack());
        assert!(!w.is_wrung());
    }

    #[test]
    fn new_clamps_max_tension() {
        let w = Wrung::new(-5.0, 1.5);
        assert!((w.max_tension - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_twist_rate() {
        let w = Wrung::new(100.0, -1.5);
        assert_eq!(w.twist_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Wrung::default();
        assert!((w.max_tension - 100.0).abs() < 1e-5);
        assert!((w.twist_rate - 1.5).abs() < 1e-5);
    }

    // --- coil ---

    #[test]
    fn coil_adds_tension() {
        let mut w = w();
        w.coil(40.0);
        assert!((w.tension - 40.0).abs() < 1e-3);
    }

    #[test]
    fn coil_clamps_at_max() {
        let mut w = w();
        w.coil(200.0);
        assert!((w.tension - 100.0).abs() < 1e-3);
    }

    #[test]
    fn coil_fires_just_wrung_at_max() {
        let mut w = w();
        w.coil(100.0);
        assert!(w.just_wrung);
        assert!(w.is_wrung());
    }

    #[test]
    fn coil_no_just_wrung_when_already_at_max() {
        let mut w = w();
        w.tension = 100.0;
        w.coil(10.0);
        assert!(!w.just_wrung);
    }

    #[test]
    fn coil_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.coil(50.0);
        assert_eq!(w.tension, 0.0);
    }

    #[test]
    fn coil_no_op_when_amount_zero() {
        let mut w = w();
        w.coil(0.0);
        assert_eq!(w.tension, 0.0);
    }

    // --- uncoil ---

    #[test]
    fn uncoil_reduces_tension() {
        let mut w = w();
        w.tension = 60.0;
        w.uncoil(20.0);
        assert!((w.tension - 40.0).abs() < 1e-3);
    }

    #[test]
    fn uncoil_clamps_at_zero() {
        let mut w = w();
        w.tension = 30.0;
        w.uncoil(200.0);
        assert_eq!(w.tension, 0.0);
    }

    #[test]
    fn uncoil_fires_just_slack_at_zero() {
        let mut w = w();
        w.tension = 30.0;
        w.uncoil(30.0);
        assert!(w.just_slack);
    }

    #[test]
    fn uncoil_no_op_when_already_slack() {
        let mut w = w();
        w.uncoil(10.0);
        assert!(!w.just_slack);
    }

    #[test]
    fn uncoil_no_op_when_disabled() {
        let mut w = w();
        w.tension = 50.0;
        w.enabled = false;
        w.uncoil(50.0);
        assert!((w.tension - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_tension() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.tension - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_wrung_on_tension_to_max() {
        let mut w = Wrung::new(100.0, 200.0);
        w.tension = 95.0;
        w.tick(1.0);
        assert!(w.just_wrung);
        assert!(w.is_wrung());
    }

    #[test]
    fn tick_no_build_when_already_wrung() {
        let mut w = w();
        w.tension = 100.0;
        w.tick(1.0);
        assert!(!w.just_wrung);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Wrung::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.tension, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.tension, 0.0);
    }

    #[test]
    fn tick_clears_just_wrung() {
        let mut w = Wrung::new(100.0, 200.0);
        w.tension = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_wrung);
    }

    #[test]
    fn tick_clears_just_slack() {
        let mut w = w();
        w.tension = 10.0;
        w.uncoil(10.0);
        w.tick(0.016);
        assert!(!w.just_slack);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.tension - 9.0).abs() < 1e-3);
    }

    // --- is_wrung / is_slack ---

    #[test]
    fn is_wrung_false_when_disabled() {
        let mut w = w();
        w.tension = 100.0;
        w.enabled = false;
        assert!(!w.is_wrung());
    }

    #[test]
    fn is_slack_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_slack());
    }

    // --- tension_fraction / effective_torque ---

    #[test]
    fn tension_fraction_zero_when_slack() {
        assert_eq!(w().tension_fraction(), 0.0);
    }

    #[test]
    fn tension_fraction_half_at_midpoint() {
        let mut w = w();
        w.tension = 50.0;
        assert!((w.tension_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_torque_zero_when_slack() {
        assert_eq!(w().effective_torque(100.0), 0.0);
    }

    #[test]
    fn effective_torque_scales_with_tension() {
        let mut w = w();
        w.tension = 75.0;
        assert!((w.effective_torque(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_torque_zero_when_disabled() {
        let mut w = w();
        w.tension = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_torque(100.0), 0.0);
    }
}
