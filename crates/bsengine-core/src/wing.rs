use bevy_ecs::prelude::Component;

/// Flight-lift accumulation tracker named after wing, the noun
/// meaning a forelimb modified for flight in birds, bats, and
/// insects; a flat or broad structure that generates lift; any
/// lateral appendage or extension — from the Old Norse vængr
/// (wing of a bird, wing of an army), from the Proto-Germanic
/// wangaz, from the Proto-Indo-European root we- (to blow, to
/// move as wind). The connection between wing and wind is
/// etymological as well as physical: the wing moves the bird
/// through the medium that wind moves through the world, and
/// the same root gave Latin ventus (wind) and spiritus (breath,
/// spirit). The wing's aerodynamic function — generating lift
/// by creating a pressure differential between its upper and
/// lower surfaces — was not understood until the Bernoulli
/// principle was formalised in the eighteenth century, but
/// birds had been exploiting it for roughly 150 million years
/// before that. Wings appear in human symbolic systems as
/// attributes of speed (winged sandals), transcendence (the
/// winged soul), protection (under the wings of providence),
/// and military organisation (the wings of an army, flanking
/// the main force). The architectural wing extends a building
/// laterally; the political wing extends a party ideologically;
/// the theatrical wing conceals the mechanisms of the stage
/// until the performer enters. In game mechanics, a wing
/// mechanic models the slow build of flight capacity — the
/// accumulation of lift, altitude, or airborne endurance that
/// eventually reaches the threshold at which sustained flight,
/// a powerful leap, or an aerial attack becomes possible.
/// `lift` builds via `soar(amount)` and accumulates passively
/// at `glide_rate` per second in `tick(dt)` or descends via
/// `stall(amount)`.
///
/// Models flight-lift fill levels, altitude-saturation bars,
/// airborne-endurance accumulators, glide-capacity gauges,
/// wing-charge fill levels, aerial-saturation indicators,
/// flight-duration accumulation bars, updraft-capture meters,
/// hover-completion fill levels, or any mechanic where a
/// character, creature, or vehicle slowly accumulates the lift,
/// altitude, or aerial energy required to achieve or sustain
/// flight — each updraft caught, each wing-beat completed, each
/// thermal ridden adding a fraction of lift until the threshold
/// is crossed and true flight becomes possible.
///
/// `soar(amount)` adds lift; fires `just_airborne` when first
/// reaching `max_lift`. No-op when disabled.
///
/// `stall(amount)` reduces lift immediately; fires `just_grounded`
/// when reaching 0. No-op when disabled or already grounded.
///
/// `tick(dt)` clears both flags, then increases lift by
/// `glide_rate * dt` (capped at `max_lift`). Fires `just_airborne`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_airborne()` returns `lift >= max_lift && enabled`.
///
/// `is_grounded()` returns `lift == 0.0` (not gated by `enabled`).
///
/// `lift_fraction()` returns `(lift / max_lift).clamp(0, 1)`.
///
/// `effective_flight(scale)` returns `scale * lift_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — glides at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wing {
    pub lift: f32,
    pub max_lift: f32,
    pub glide_rate: f32,
    pub just_airborne: bool,
    pub just_grounded: bool,
    pub enabled: bool,
}

impl Wing {
    pub fn new(max_lift: f32, glide_rate: f32) -> Self {
        Self {
            lift: 0.0,
            max_lift: max_lift.max(0.1),
            glide_rate: glide_rate.max(0.0),
            just_airborne: false,
            just_grounded: false,
            enabled: true,
        }
    }

    /// Add lift; fires `just_airborne` when first reaching max.
    /// No-op when disabled.
    pub fn soar(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.lift < self.max_lift;
        self.lift = (self.lift + amount).min(self.max_lift);
        if was_below && self.lift >= self.max_lift {
            self.just_airborne = true;
        }
    }

    /// Reduce lift; fires `just_grounded` when reaching 0.
    /// No-op when disabled or already grounded.
    pub fn stall(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.lift <= 0.0 {
            return;
        }
        self.lift = (self.lift - amount).max(0.0);
        if self.lift <= 0.0 {
            self.just_grounded = true;
        }
    }

    /// Clear flags, then increase lift by `glide_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_airborne = false;
        self.just_grounded = false;
        if self.enabled && self.glide_rate > 0.0 && self.lift < self.max_lift {
            let was_below = self.lift < self.max_lift;
            self.lift = (self.lift + self.glide_rate * dt).min(self.max_lift);
            if was_below && self.lift >= self.max_lift {
                self.just_airborne = true;
            }
        }
    }

    /// `true` when lift is at maximum and component is enabled.
    pub fn is_airborne(&self) -> bool {
        self.lift >= self.max_lift && self.enabled
    }

    /// `true` when lift is 0 (not gated by `enabled`).
    pub fn is_grounded(&self) -> bool {
        self.lift == 0.0
    }

    /// Fraction of maximum lift [0.0, 1.0].
    pub fn lift_fraction(&self) -> f32 {
        (self.lift / self.max_lift).clamp(0.0, 1.0)
    }

    /// Returns `scale * lift_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_flight(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.lift_fraction()
    }
}

impl Default for Wing {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wing {
        Wing::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_grounded() {
        let w = w();
        assert_eq!(w.lift, 0.0);
        assert!(w.is_grounded());
        assert!(!w.is_airborne());
    }

    #[test]
    fn new_clamps_max_lift() {
        let w = Wing::new(-5.0, 1.5);
        assert!((w.max_lift - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_glide_rate() {
        let w = Wing::new(100.0, -1.5);
        assert_eq!(w.glide_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Wing::default();
        assert!((w.max_lift - 100.0).abs() < 1e-5);
        assert!((w.glide_rate - 1.5).abs() < 1e-5);
    }

    // --- soar ---

    #[test]
    fn soar_adds_lift() {
        let mut w = w();
        w.soar(40.0);
        assert!((w.lift - 40.0).abs() < 1e-3);
    }

    #[test]
    fn soar_clamps_at_max() {
        let mut w = w();
        w.soar(200.0);
        assert!((w.lift - 100.0).abs() < 1e-3);
    }

    #[test]
    fn soar_fires_just_airborne_at_max() {
        let mut w = w();
        w.soar(100.0);
        assert!(w.just_airborne);
        assert!(w.is_airborne());
    }

    #[test]
    fn soar_no_just_airborne_when_already_at_max() {
        let mut w = w();
        w.lift = 100.0;
        w.soar(10.0);
        assert!(!w.just_airborne);
    }

    #[test]
    fn soar_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.soar(50.0);
        assert_eq!(w.lift, 0.0);
    }

    #[test]
    fn soar_no_op_when_amount_zero() {
        let mut w = w();
        w.soar(0.0);
        assert_eq!(w.lift, 0.0);
    }

    // --- stall ---

    #[test]
    fn stall_reduces_lift() {
        let mut w = w();
        w.lift = 60.0;
        w.stall(20.0);
        assert!((w.lift - 40.0).abs() < 1e-3);
    }

    #[test]
    fn stall_clamps_at_zero() {
        let mut w = w();
        w.lift = 30.0;
        w.stall(200.0);
        assert_eq!(w.lift, 0.0);
    }

    #[test]
    fn stall_fires_just_grounded_at_zero() {
        let mut w = w();
        w.lift = 30.0;
        w.stall(30.0);
        assert!(w.just_grounded);
    }

    #[test]
    fn stall_no_op_when_already_grounded() {
        let mut w = w();
        w.stall(10.0);
        assert!(!w.just_grounded);
    }

    #[test]
    fn stall_no_op_when_disabled() {
        let mut w = w();
        w.lift = 50.0;
        w.enabled = false;
        w.stall(50.0);
        assert!((w.lift - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_lift() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.lift - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_airborne_on_lift_to_max() {
        let mut w = Wing::new(100.0, 200.0);
        w.lift = 95.0;
        w.tick(1.0);
        assert!(w.just_airborne);
        assert!(w.is_airborne());
    }

    #[test]
    fn tick_no_build_when_already_airborne() {
        let mut w = w();
        w.lift = 100.0;
        w.tick(1.0);
        assert!(!w.just_airborne);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Wing::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.lift, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.lift, 0.0);
    }

    #[test]
    fn tick_clears_just_airborne() {
        let mut w = Wing::new(100.0, 200.0);
        w.lift = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_airborne);
    }

    #[test]
    fn tick_clears_just_grounded() {
        let mut w = w();
        w.lift = 10.0;
        w.stall(10.0);
        w.tick(0.016);
        assert!(!w.just_grounded);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.lift - 9.0).abs() < 1e-3);
    }

    // --- is_airborne / is_grounded ---

    #[test]
    fn is_airborne_false_when_disabled() {
        let mut w = w();
        w.lift = 100.0;
        w.enabled = false;
        assert!(!w.is_airborne());
    }

    #[test]
    fn is_grounded_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_grounded());
    }

    // --- lift_fraction / effective_flight ---

    #[test]
    fn lift_fraction_zero_when_grounded() {
        assert_eq!(w().lift_fraction(), 0.0);
    }

    #[test]
    fn lift_fraction_half_at_midpoint() {
        let mut w = w();
        w.lift = 50.0;
        assert!((w.lift_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_flight_zero_when_grounded() {
        assert_eq!(w().effective_flight(100.0), 0.0);
    }

    #[test]
    fn effective_flight_scales_with_lift() {
        let mut w = w();
        w.lift = 75.0;
        assert!((w.effective_flight(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_flight_zero_when_disabled() {
        let mut w = w();
        w.lift = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_flight(100.0), 0.0);
    }
}
