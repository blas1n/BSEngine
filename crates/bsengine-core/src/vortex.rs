use bevy_ecs::prelude::Component;

/// Rotational-pull accumulation tracker named after vortex, the noun
/// meaning a mass of fluid (especially water or air) rotating rapidly
/// around a central point, especially one that draws objects toward
/// its centre — from the Latin vortex or vertex, meaning a whirlpool,
/// eddy, from vertere (to turn). The vortex entered scientific usage
/// as the key concept in Cartesian cosmology: Descartes proposed that
/// the planets were carried around the sun by vast invisible vortices
/// of subtle matter, each planet embedded in its own rotating ring
/// of cosmic fluid. Newton's physics displaced the vortex theory, but
/// the word itself survived into fluid dynamics where it names any
/// region of fluid with a closed or spiral streamline — the tip vortex
/// shed by an aircraft wing, the polar vortex of atmospheric
/// circulation, the bathtub vortex that forms as water drains, the
/// vortex street that trails behind a cylinder in crossflow. The
/// word carries with it the connotation of irresistible pull: the
/// vortex draws things in, and once drawn in, they cannot easily
/// escape. The political vortex, the media vortex, the gravitational
/// vortex — all share this structure of a centre that concentrates
/// influence or material by virtue of its rotation. In game mechanics,
/// a vortex mechanic models the accumulation of rotational energy
/// or gravitational pull — the slow build of a spiralling force that
/// eventually reaches a threshold of suction beyond which nearby
/// objects are pulled helplessly toward the centre. `spin` builds via
/// `wind(amount)` and accumulates passively at `spiral_rate` per
/// second in `tick(dt)` or dissipates via `dissipate(amount)`.
///
/// Models rotational-pull fill levels, suction-power saturation bars,
/// whirlpool-strength accumulators, tornado-intensity gauges, drain-
/// pull fill levels, gravitational-spiral saturation indicators,
/// black-hole-approach accumulation bars, maelstrom-power meters,
/// cyclone-formation fill levels, or any mechanic where a rotating
/// force slowly accumulates angular momentum and suction power until
/// a threshold is reached and the vortex becomes fully formed —
/// drawing in debris, enemies, or projectiles with a pull that nothing
/// in range can resist until the spin is finally exhausted.
///
/// `wind(amount)` adds spin; fires `just_formed` when first reaching
/// `max_spin`. No-op when disabled.
///
/// `dissipate(amount)` reduces spin immediately; fires `just_calm`
/// when reaching 0. No-op when disabled or already calm.
///
/// `tick(dt)` clears both flags, then increases spin by
/// `spiral_rate * dt` (capped at `max_spin`). Fires `just_formed`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_formed()` returns `spin >= max_spin && enabled`.
///
/// `is_calm()` returns `spin == 0.0` (not gated by `enabled`).
///
/// `spin_fraction()` returns `(spin / max_spin).clamp(0, 1)`.
///
/// `effective_pull(scale)` returns `scale * spin_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — spirals at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Vortex {
    pub spin: f32,
    pub max_spin: f32,
    pub spiral_rate: f32,
    pub just_formed: bool,
    pub just_calm: bool,
    pub enabled: bool,
}

impl Vortex {
    pub fn new(max_spin: f32, spiral_rate: f32) -> Self {
        Self {
            spin: 0.0,
            max_spin: max_spin.max(0.1),
            spiral_rate: spiral_rate.max(0.0),
            just_formed: false,
            just_calm: false,
            enabled: true,
        }
    }

    /// Add spin; fires `just_formed` when first reaching max.
    /// No-op when disabled.
    pub fn wind(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.spin < self.max_spin;
        self.spin = (self.spin + amount).min(self.max_spin);
        if was_below && self.spin >= self.max_spin {
            self.just_formed = true;
        }
    }

    /// Reduce spin; fires `just_calm` when reaching 0.
    /// No-op when disabled or already calm.
    pub fn dissipate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.spin <= 0.0 {
            return;
        }
        self.spin = (self.spin - amount).max(0.0);
        if self.spin <= 0.0 {
            self.just_calm = true;
        }
    }

    /// Clear flags, then increase spin by `spiral_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_formed = false;
        self.just_calm = false;
        if self.enabled && self.spiral_rate > 0.0 && self.spin < self.max_spin {
            let was_below = self.spin < self.max_spin;
            self.spin = (self.spin + self.spiral_rate * dt).min(self.max_spin);
            if was_below && self.spin >= self.max_spin {
                self.just_formed = true;
            }
        }
    }

    /// `true` when spin is at maximum and component is enabled.
    pub fn is_formed(&self) -> bool {
        self.spin >= self.max_spin && self.enabled
    }

    /// `true` when spin is 0 (not gated by `enabled`).
    pub fn is_calm(&self) -> bool {
        self.spin == 0.0
    }

    /// Fraction of maximum spin [0.0, 1.0].
    pub fn spin_fraction(&self) -> f32 {
        (self.spin / self.max_spin).clamp(0.0, 1.0)
    }

    /// Returns `scale * spin_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_pull(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.spin_fraction()
    }
}

impl Default for Vortex {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v() -> Vortex {
        Vortex::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_calm() {
        let v = v();
        assert_eq!(v.spin, 0.0);
        assert!(v.is_calm());
        assert!(!v.is_formed());
    }

    #[test]
    fn new_clamps_max_spin() {
        let v = Vortex::new(-5.0, 1.5);
        assert!((v.max_spin - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_spiral_rate() {
        let v = Vortex::new(100.0, -1.5);
        assert_eq!(v.spiral_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let v = Vortex::default();
        assert!((v.max_spin - 100.0).abs() < 1e-5);
        assert!((v.spiral_rate - 1.5).abs() < 1e-5);
    }

    // --- wind ---

    #[test]
    fn wind_adds_spin() {
        let mut v = v();
        v.wind(40.0);
        assert!((v.spin - 40.0).abs() < 1e-3);
    }

    #[test]
    fn wind_clamps_at_max() {
        let mut v = v();
        v.wind(200.0);
        assert!((v.spin - 100.0).abs() < 1e-3);
    }

    #[test]
    fn wind_fires_just_formed_at_max() {
        let mut v = v();
        v.wind(100.0);
        assert!(v.just_formed);
        assert!(v.is_formed());
    }

    #[test]
    fn wind_no_just_formed_when_already_at_max() {
        let mut v = v();
        v.spin = 100.0;
        v.wind(10.0);
        assert!(!v.just_formed);
    }

    #[test]
    fn wind_no_op_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.wind(50.0);
        assert_eq!(v.spin, 0.0);
    }

    #[test]
    fn wind_no_op_when_amount_zero() {
        let mut v = v();
        v.wind(0.0);
        assert_eq!(v.spin, 0.0);
    }

    // --- dissipate ---

    #[test]
    fn dissipate_reduces_spin() {
        let mut v = v();
        v.spin = 60.0;
        v.dissipate(20.0);
        assert!((v.spin - 40.0).abs() < 1e-3);
    }

    #[test]
    fn dissipate_clamps_at_zero() {
        let mut v = v();
        v.spin = 30.0;
        v.dissipate(200.0);
        assert_eq!(v.spin, 0.0);
    }

    #[test]
    fn dissipate_fires_just_calm_at_zero() {
        let mut v = v();
        v.spin = 30.0;
        v.dissipate(30.0);
        assert!(v.just_calm);
    }

    #[test]
    fn dissipate_no_op_when_already_calm() {
        let mut v = v();
        v.dissipate(10.0);
        assert!(!v.just_calm);
    }

    #[test]
    fn dissipate_no_op_when_disabled() {
        let mut v = v();
        v.spin = 50.0;
        v.enabled = false;
        v.dissipate(50.0);
        assert!((v.spin - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_spin() {
        let mut v = v(); // rate=1.5
        v.tick(4.0); // 0 + 1.5*4 = 6
        assert!((v.spin - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_formed_on_spin_to_max() {
        let mut v = Vortex::new(100.0, 200.0);
        v.spin = 95.0;
        v.tick(1.0);
        assert!(v.just_formed);
        assert!(v.is_formed());
    }

    #[test]
    fn tick_no_build_when_already_formed() {
        let mut v = v();
        v.spin = 100.0;
        v.tick(1.0);
        assert!(!v.just_formed);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut v = Vortex::new(100.0, 0.0);
        v.tick(100.0);
        assert_eq!(v.spin, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.tick(1.0);
        assert_eq!(v.spin, 0.0);
    }

    #[test]
    fn tick_clears_just_formed() {
        let mut v = Vortex::new(100.0, 200.0);
        v.spin = 95.0;
        v.tick(1.0);
        v.tick(0.016);
        assert!(!v.just_formed);
    }

    #[test]
    fn tick_clears_just_calm() {
        let mut v = v();
        v.spin = 10.0;
        v.dissipate(10.0);
        v.tick(0.016);
        assert!(!v.just_calm);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut v = v(); // rate=1.5
        v.tick(6.0); // 1.5*6 = 9
        assert!((v.spin - 9.0).abs() < 1e-3);
    }

    // --- is_formed / is_calm ---

    #[test]
    fn is_formed_false_when_disabled() {
        let mut v = v();
        v.spin = 100.0;
        v.enabled = false;
        assert!(!v.is_formed());
    }

    #[test]
    fn is_calm_not_gated_by_enabled() {
        let mut v = v();
        v.enabled = false;
        assert!(v.is_calm());
    }

    // --- spin_fraction / effective_pull ---

    #[test]
    fn spin_fraction_zero_when_calm() {
        assert_eq!(v().spin_fraction(), 0.0);
    }

    #[test]
    fn spin_fraction_half_at_midpoint() {
        let mut v = v();
        v.spin = 50.0;
        assert!((v.spin_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_pull_zero_when_calm() {
        assert_eq!(v().effective_pull(100.0), 0.0);
    }

    #[test]
    fn effective_pull_scales_with_spin() {
        let mut v = v();
        v.spin = 75.0;
        assert!((v.effective_pull(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_pull_zero_when_disabled() {
        let mut v = v();
        v.spin = 50.0;
        v.enabled = false;
        assert_eq!(v.effective_pull(100.0), 0.0);
    }
}
