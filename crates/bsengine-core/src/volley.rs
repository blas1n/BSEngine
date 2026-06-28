use bevy_ecs::prelude::Component;

/// Projectile-burst accumulation tracker named after volley, the noun
/// meaning a flight of missiles (arrows, bullets, or other projectiles)
/// discharged simultaneously or in rapid succession — from the Old
/// French volee, meaning a flight, from voler (to fly), from the Latin
/// volare. The word entered English military usage in the sixteenth
/// century when the development of firearms made simultaneous discharge
/// tactically significant: a volley of musket fire was the basic unit
/// of early modern infantry combat, delivered on command at close range
/// to compensate for the inaccuracy of smooth-bore weapons with the
/// sheer density of projectiles. The volley system required strict
/// discipline and timing — soldiers loaded in sequence, presented arms
/// on command, and fired together — because the military value of the
/// simultaneous discharge depended entirely on the simultaneity: a
/// staggered volley became merely random fire. In tennis and other
/// racket sports the volley is a stroke played before the ball bounces,
/// taking the ball out of the air — the fastest, most aggressive
/// response in the game, requiring reflexes and court position rather
/// than the patience of a baseline rally. The word also entered legal
/// usage as a volley of questions — a rapid succession of demands that
/// overwhelms the respondent before they can answer any single one. In
/// game mechanics, volley energy models the slow accumulation of charge
/// required to release a burst — the gathering of projectiles, the
/// build-up of pressure, the tension before the simultaneous release.
/// `charge` builds via `load(amount)` and accumulates passively at
/// `draw_rate` per second in `tick(dt)` or is released via
/// `release(amount)`.
///
/// Models projectile-burst fill levels, multi-shot saturation bars,
/// arrow-volley accumulators, bullet-barrage gauges, simultaneous-fire
/// fill levels, burst-mode saturation indicators, charged-shot
/// accumulation bars, rapid-succession meters, fusillade-preparation
/// fill levels, or any mechanic where a weapon, turret, tower, or
/// ability slowly accumulates the charge or ammunition required for a
/// burst discharge — loading round by round, drawing string by string,
/// pressurising chamber by chamber — until the threshold is reached and
/// the volley can be released in one devastating simultaneous discharge.
///
/// `load(amount)` adds charge; fires `just_vollied` when first
/// reaching `max_charge`. No-op when disabled.
///
/// `release(amount)` reduces charge immediately; fires `just_spent`
/// when reaching 0. No-op when disabled or already spent.
///
/// `tick(dt)` clears both flags, then increases charge by
/// `draw_rate * dt` (capped at `max_charge`). Fires `just_vollied`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_vollied()` returns `charge >= max_charge && enabled`.
///
/// `is_spent()` returns `charge == 0.0` (not gated by `enabled`).
///
/// `charge_fraction()` returns `(charge / max_charge).clamp(0, 1)`.
///
/// `effective_burst(scale)` returns `scale * charge_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — draws at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Volley {
    pub charge: f32,
    pub max_charge: f32,
    pub draw_rate: f32,
    pub just_vollied: bool,
    pub just_spent: bool,
    pub enabled: bool,
}

impl Volley {
    pub fn new(max_charge: f32, draw_rate: f32) -> Self {
        Self {
            charge: 0.0,
            max_charge: max_charge.max(0.1),
            draw_rate: draw_rate.max(0.0),
            just_vollied: false,
            just_spent: false,
            enabled: true,
        }
    }

    /// Add charge; fires `just_vollied` when first reaching max.
    /// No-op when disabled.
    pub fn load(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.charge < self.max_charge;
        self.charge = (self.charge + amount).min(self.max_charge);
        if was_below && self.charge >= self.max_charge {
            self.just_vollied = true;
        }
    }

    /// Reduce charge; fires `just_spent` when reaching 0.
    /// No-op when disabled or already spent.
    pub fn release(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.charge <= 0.0 {
            return;
        }
        self.charge = (self.charge - amount).max(0.0);
        if self.charge <= 0.0 {
            self.just_spent = true;
        }
    }

    /// Clear flags, then increase charge by `draw_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_vollied = false;
        self.just_spent = false;
        if self.enabled && self.draw_rate > 0.0 && self.charge < self.max_charge {
            let was_below = self.charge < self.max_charge;
            self.charge = (self.charge + self.draw_rate * dt).min(self.max_charge);
            if was_below && self.charge >= self.max_charge {
                self.just_vollied = true;
            }
        }
    }

    /// `true` when charge is at maximum and component is enabled.
    pub fn is_vollied(&self) -> bool {
        self.charge >= self.max_charge && self.enabled
    }

    /// `true` when charge is 0 (not gated by `enabled`).
    pub fn is_spent(&self) -> bool {
        self.charge == 0.0
    }

    /// Fraction of maximum charge [0.0, 1.0].
    pub fn charge_fraction(&self) -> f32 {
        (self.charge / self.max_charge).clamp(0.0, 1.0)
    }

    /// Returns `scale * charge_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_burst(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.charge_fraction()
    }
}

impl Default for Volley {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v() -> Volley {
        Volley::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_spent() {
        let v = v();
        assert_eq!(v.charge, 0.0);
        assert!(v.is_spent());
        assert!(!v.is_vollied());
    }

    #[test]
    fn new_clamps_max_charge() {
        let v = Volley::new(-5.0, 1.5);
        assert!((v.max_charge - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_draw_rate() {
        let v = Volley::new(100.0, -1.5);
        assert_eq!(v.draw_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let v = Volley::default();
        assert!((v.max_charge - 100.0).abs() < 1e-5);
        assert!((v.draw_rate - 1.5).abs() < 1e-5);
    }

    // --- load ---

    #[test]
    fn load_adds_charge() {
        let mut v = v();
        v.load(40.0);
        assert!((v.charge - 40.0).abs() < 1e-3);
    }

    #[test]
    fn load_clamps_at_max() {
        let mut v = v();
        v.load(200.0);
        assert!((v.charge - 100.0).abs() < 1e-3);
    }

    #[test]
    fn load_fires_just_vollied_at_max() {
        let mut v = v();
        v.load(100.0);
        assert!(v.just_vollied);
        assert!(v.is_vollied());
    }

    #[test]
    fn load_no_just_vollied_when_already_at_max() {
        let mut v = v();
        v.charge = 100.0;
        v.load(10.0);
        assert!(!v.just_vollied);
    }

    #[test]
    fn load_no_op_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.load(50.0);
        assert_eq!(v.charge, 0.0);
    }

    #[test]
    fn load_no_op_when_amount_zero() {
        let mut v = v();
        v.load(0.0);
        assert_eq!(v.charge, 0.0);
    }

    // --- release ---

    #[test]
    fn release_reduces_charge() {
        let mut v = v();
        v.charge = 60.0;
        v.release(20.0);
        assert!((v.charge - 40.0).abs() < 1e-3);
    }

    #[test]
    fn release_clamps_at_zero() {
        let mut v = v();
        v.charge = 30.0;
        v.release(200.0);
        assert_eq!(v.charge, 0.0);
    }

    #[test]
    fn release_fires_just_spent_at_zero() {
        let mut v = v();
        v.charge = 30.0;
        v.release(30.0);
        assert!(v.just_spent);
    }

    #[test]
    fn release_no_op_when_already_spent() {
        let mut v = v();
        v.release(10.0);
        assert!(!v.just_spent);
    }

    #[test]
    fn release_no_op_when_disabled() {
        let mut v = v();
        v.charge = 50.0;
        v.enabled = false;
        v.release(50.0);
        assert!((v.charge - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_charge() {
        let mut v = v(); // rate=1.5
        v.tick(4.0); // 0 + 1.5*4 = 6
        assert!((v.charge - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_vollied_on_charge_to_max() {
        let mut v = Volley::new(100.0, 200.0);
        v.charge = 95.0;
        v.tick(1.0);
        assert!(v.just_vollied);
        assert!(v.is_vollied());
    }

    #[test]
    fn tick_no_build_when_already_vollied() {
        let mut v = v();
        v.charge = 100.0;
        v.tick(1.0);
        assert!(!v.just_vollied);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut v = Volley::new(100.0, 0.0);
        v.tick(100.0);
        assert_eq!(v.charge, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.tick(1.0);
        assert_eq!(v.charge, 0.0);
    }

    #[test]
    fn tick_clears_just_vollied() {
        let mut v = Volley::new(100.0, 200.0);
        v.charge = 95.0;
        v.tick(1.0);
        v.tick(0.016);
        assert!(!v.just_vollied);
    }

    #[test]
    fn tick_clears_just_spent() {
        let mut v = v();
        v.charge = 10.0;
        v.release(10.0);
        v.tick(0.016);
        assert!(!v.just_spent);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut v = v(); // rate=1.5
        v.tick(6.0); // 1.5*6 = 9
        assert!((v.charge - 9.0).abs() < 1e-3);
    }

    // --- is_vollied / is_spent ---

    #[test]
    fn is_vollied_false_when_disabled() {
        let mut v = v();
        v.charge = 100.0;
        v.enabled = false;
        assert!(!v.is_vollied());
    }

    #[test]
    fn is_spent_not_gated_by_enabled() {
        let mut v = v();
        v.enabled = false;
        assert!(v.is_spent());
    }

    // --- charge_fraction / effective_burst ---

    #[test]
    fn charge_fraction_zero_when_spent() {
        assert_eq!(v().charge_fraction(), 0.0);
    }

    #[test]
    fn charge_fraction_half_at_midpoint() {
        let mut v = v();
        v.charge = 50.0;
        assert!((v.charge_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_burst_zero_when_spent() {
        assert_eq!(v().effective_burst(100.0), 0.0);
    }

    #[test]
    fn effective_burst_scales_with_charge() {
        let mut v = v();
        v.charge = 75.0;
        assert!((v.effective_burst(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_burst_zero_when_disabled() {
        let mut v = v();
        v.charge = 50.0;
        v.enabled = false;
        assert_eq!(v.effective_burst(100.0), 0.0);
    }
}
