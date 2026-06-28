use bevy_ecs::prelude::Component;

/// Ciliary-zonule fiber tension tracker named after the zonule (from
/// Latin zonula, "small belt"), the ring of slender radial fibres —
/// also called the zonule of Zinn or ciliary zonule — that suspend the
/// crystalline lens inside the eye. Each fibre runs from the ciliary
/// body to the lens equator; together they form a scaffolding that
/// holds the lens in precise alignment with the optical axis while
/// transmitting the mechanical forces needed for accommodation. When the
/// ciliary muscle relaxes, the choroid pulls the ciliary body backward,
/// the fibres tauten, and the lens is stretched flat: the eye resolves
/// distant objects. When the ciliary muscle contracts, the ciliary body
/// moves inward, fibre tension drops, and the elastic capsule of the
/// lens allows it to round up into a more convex shape: the eye
/// converges to a near focal point. The process is called the Helmholtz
/// theory of accommodation. Disruption of zonular fibres — whether
/// from age-related pseudoexfoliation, genetic defects in fibrillin-1,
/// or trauma — causes lens subluxation (ectopia lentis) and is the
/// defining clinical feature of Marfan syndrome and Weill–Marchesani
/// syndrome. `tension` builds via `cinch(amount)` and accumulates
/// passively at `cinch_rate` per second in `tick(dt)` or is relieved
/// via `slacken(amount)`.
///
/// Models ciliary-fibre tension fill levels, lens-suspension tautness
/// bars, optical-accommodation saturation trackers, Helmholtz-
/// accommodation strain gauges, crystalline-lens-flattening meters,
/// fibrillin-scaffolding integrity fill levels, visual-focus-drive
/// tension bars, intraocular-mechanical-coupling saturation meters,
/// zonular-stress fill levels for ageing-lens simulations, or any
/// mechanic where a radial array of invisible elastic cables must be
/// kept at precisely the right tension to hold a delicate optical
/// element in focus — and where too much slack or too many broken
/// strands sends everything spinning into uncorrectable blur.
///
/// `cinch(amount)` adds tension; fires `just_taut` when first reaching
/// `max_tension`. No-op when disabled.
///
/// `slacken(amount)` reduces tension immediately; fires `just_slack`
/// when reaching 0. No-op when disabled or already slack.
///
/// `tick(dt)` clears both flags, then increases tension by
/// `cinch_rate * dt` (capped at `max_tension`). Fires `just_taut`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_taut()` returns `tension >= max_tension && enabled`.
///
/// `is_slack()` returns `tension == 0.0` (not gated by `enabled`).
///
/// `tension_fraction()` returns `(tension / max_tension).clamp(0, 1)`.
///
/// `effective_suspension(scale)` returns `scale * tension_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — cinches at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zonule {
    pub tension: f32,
    pub max_tension: f32,
    pub cinch_rate: f32,
    pub just_taut: bool,
    pub just_slack: bool,
    pub enabled: bool,
}

impl Zonule {
    pub fn new(max_tension: f32, cinch_rate: f32) -> Self {
        Self {
            tension: 0.0,
            max_tension: max_tension.max(0.1),
            cinch_rate: cinch_rate.max(0.0),
            just_taut: false,
            just_slack: false,
            enabled: true,
        }
    }

    /// Add tension; fires `just_taut` when first reaching max.
    /// No-op when disabled.
    pub fn cinch(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.tension < self.max_tension;
        self.tension = (self.tension + amount).min(self.max_tension);
        if was_below && self.tension >= self.max_tension {
            self.just_taut = true;
        }
    }

    /// Reduce tension; fires `just_slack` when reaching 0.
    /// No-op when disabled or already slack.
    pub fn slacken(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.tension <= 0.0 {
            return;
        }
        self.tension = (self.tension - amount).max(0.0);
        if self.tension <= 0.0 {
            self.just_slack = true;
        }
    }

    /// Clear flags, then increase tension by `cinch_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_taut = false;
        self.just_slack = false;
        if self.enabled && self.cinch_rate > 0.0 && self.tension < self.max_tension {
            let was_below = self.tension < self.max_tension;
            self.tension = (self.tension + self.cinch_rate * dt).min(self.max_tension);
            if was_below && self.tension >= self.max_tension {
                self.just_taut = true;
            }
        }
    }

    /// `true` when tension is at maximum and component is enabled.
    pub fn is_taut(&self) -> bool {
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
    pub fn effective_suspension(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.tension_fraction()
    }
}

impl Default for Zonule {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zonule {
        Zonule::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_slack() {
        let z = z();
        assert_eq!(z.tension, 0.0);
        assert!(z.is_slack());
        assert!(!z.is_taut());
    }

    #[test]
    fn new_clamps_max_tension() {
        let z = Zonule::new(-5.0, 1.5);
        assert!((z.max_tension - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_cinch_rate() {
        let z = Zonule::new(100.0, -1.5);
        assert_eq!(z.cinch_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zonule::default();
        assert!((z.max_tension - 100.0).abs() < 1e-5);
        assert!((z.cinch_rate - 1.5).abs() < 1e-5);
    }

    // --- cinch ---

    #[test]
    fn cinch_adds_tension() {
        let mut z = z();
        z.cinch(40.0);
        assert!((z.tension - 40.0).abs() < 1e-3);
    }

    #[test]
    fn cinch_clamps_at_max() {
        let mut z = z();
        z.cinch(200.0);
        assert!((z.tension - 100.0).abs() < 1e-3);
    }

    #[test]
    fn cinch_fires_just_taut_at_max() {
        let mut z = z();
        z.cinch(100.0);
        assert!(z.just_taut);
        assert!(z.is_taut());
    }

    #[test]
    fn cinch_no_just_taut_when_already_at_max() {
        let mut z = z();
        z.tension = 100.0;
        z.cinch(10.0);
        assert!(!z.just_taut);
    }

    #[test]
    fn cinch_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.cinch(50.0);
        assert_eq!(z.tension, 0.0);
    }

    #[test]
    fn cinch_no_op_when_amount_zero() {
        let mut z = z();
        z.cinch(0.0);
        assert_eq!(z.tension, 0.0);
    }

    // --- slacken ---

    #[test]
    fn slacken_reduces_tension() {
        let mut z = z();
        z.tension = 60.0;
        z.slacken(20.0);
        assert!((z.tension - 40.0).abs() < 1e-3);
    }

    #[test]
    fn slacken_clamps_at_zero() {
        let mut z = z();
        z.tension = 30.0;
        z.slacken(200.0);
        assert_eq!(z.tension, 0.0);
    }

    #[test]
    fn slacken_fires_just_slack_at_zero() {
        let mut z = z();
        z.tension = 30.0;
        z.slacken(30.0);
        assert!(z.just_slack);
    }

    #[test]
    fn slacken_no_op_when_already_slack() {
        let mut z = z();
        z.slacken(10.0);
        assert!(!z.just_slack);
    }

    #[test]
    fn slacken_no_op_when_disabled() {
        let mut z = z();
        z.tension = 50.0;
        z.enabled = false;
        z.slacken(50.0);
        assert!((z.tension - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_cinches_tension() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.tension - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_taut_on_cinch_to_max() {
        let mut z = Zonule::new(100.0, 200.0);
        z.tension = 95.0;
        z.tick(1.0);
        assert!(z.just_taut);
        assert!(z.is_taut());
    }

    #[test]
    fn tick_no_cinch_when_already_taut() {
        let mut z = z();
        z.tension = 100.0;
        z.tick(1.0);
        assert!(!z.just_taut);
    }

    #[test]
    fn tick_no_cinch_when_rate_zero() {
        let mut z = Zonule::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.tension, 0.0);
    }

    #[test]
    fn tick_no_cinch_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.tension, 0.0);
    }

    #[test]
    fn tick_clears_just_taut() {
        let mut z = Zonule::new(100.0, 200.0);
        z.tension = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_taut);
    }

    #[test]
    fn tick_clears_just_slack() {
        let mut z = z();
        z.tension = 10.0;
        z.slacken(10.0);
        z.tick(0.016);
        assert!(!z.just_slack);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.tension - 9.0).abs() < 1e-3);
    }

    // --- is_taut / is_slack ---

    #[test]
    fn is_taut_false_when_disabled() {
        let mut z = z();
        z.tension = 100.0;
        z.enabled = false;
        assert!(!z.is_taut());
    }

    #[test]
    fn is_slack_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_slack());
    }

    // --- tension_fraction / effective_suspension ---

    #[test]
    fn tension_fraction_zero_when_slack() {
        assert_eq!(z().tension_fraction(), 0.0);
    }

    #[test]
    fn tension_fraction_half_at_midpoint() {
        let mut z = z();
        z.tension = 50.0;
        assert!((z.tension_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_suspension_zero_when_slack() {
        assert_eq!(z().effective_suspension(100.0), 0.0);
    }

    #[test]
    fn effective_suspension_scales_with_tension() {
        let mut z = z();
        z.tension = 75.0;
        assert!((z.effective_suspension(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_suspension_zero_when_disabled() {
        let mut z = z();
        z.tension = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_suspension(100.0), 0.0);
    }
}
