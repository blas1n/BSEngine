use bevy_ecs::prelude::Component;

/// Decay-or-accumulate tension counter driven by incoming hit frequency.
/// `strife` [0.0, `max_strife`] builds from repeated hits and decays when
/// combat pressure drops. At `max_strife` the entity enters an enraged
/// state where `damage_bonus()` returns a full multiplier.
///
/// `hit()` adds `gain_per_hit` to `strife` (capped at `max_strife`) and
/// fires `just_peaked` on the transition that first reaches `max_strife`.
/// No-op when disabled.
///
/// `tick(dt)` clears `just_peaked` at the start, then decays `strife` by
/// `decay_rate * dt` (floored at 0.0).
///
/// `damage_bonus(base)` returns `base * (1 + strife_fraction())` when
/// enabled — at full strife the entity deals up to 2× base. At 0 strife
/// there is no bonus.
///
/// Distinct from `Fury` (scales continuously with HP-loss ratio — more
/// HP lost = higher fury), `Rage` (discrete triggered anger state with a
/// single activation), and `Galvanize` (charge meter specifically from
/// taking damage, aimed at a single burst release): Strife is a
/// **sustain-in-combat tension accumulator** — it rewards staying in
/// the fight under pressure and penalises pulling back, without caring
/// about HP or a single cooldown.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Strife {
    /// Current tension level [0.0, max_strife].
    pub strife: f32,
    /// Maximum strife before enraged state. Clamped ≥ 1.0.
    pub max_strife: f32,
    /// Strife gained per incoming hit. Clamped ≥ 0.0.
    pub gain_per_hit: f32,
    /// Strife lost per second while not being hit. Clamped ≥ 0.0.
    pub decay_rate: f32,
    pub just_peaked: bool,
    pub enabled: bool,
}

impl Strife {
    pub fn new(max_strife: f32, gain_per_hit: f32, decay_rate: f32) -> Self {
        Self {
            strife: 0.0,
            max_strife: max_strife.max(1.0),
            gain_per_hit: gain_per_hit.max(0.0),
            decay_rate: decay_rate.max(0.0),
            just_peaked: false,
            enabled: true,
        }
    }

    /// Register an incoming hit: add `gain_per_hit` to `strife` (capped at
    /// `max_strife`). Fires `just_peaked` on the first transition to
    /// `max_strife`. No-op when disabled.
    pub fn hit(&mut self) {
        if !self.enabled {
            return;
        }
        let was_below_peak = !self.is_enraged();
        self.strife = (self.strife + self.gain_per_hit).min(self.max_strife);
        if was_below_peak && self.is_enraged() {
            self.just_peaked = true;
        }
    }

    /// Clear one-frame flags, then decay `strife` by `decay_rate * dt`.
    /// Strife is floored at 0.0.
    pub fn tick(&mut self, dt: f32) {
        self.just_peaked = false;

        if self.strife > 0.0 && self.decay_rate > 0.0 {
            self.strife = (self.strife - self.decay_rate * dt).max(0.0);
        }
    }

    /// `true` when strife has reached `max_strife` and the component is enabled.
    pub fn is_enraged(&self) -> bool {
        self.strife >= self.max_strife && self.enabled
    }

    /// Strife fill fraction [0.0 = none, 1.0 = enraged]. Always in [0, 1].
    pub fn strife_fraction(&self) -> f32 {
        (self.strife / self.max_strife).clamp(0.0, 1.0)
    }

    /// Effective outgoing damage scaled by current strife fraction.
    /// Returns `base * (1 + strife_fraction())` when enabled — at full
    /// strife the entity deals up to 2× base. Returns `base` when
    /// disabled or strife is 0.
    pub fn damage_bonus(&self, base: f32) -> f32 {
        if self.enabled && self.strife > 0.0 {
            base * (1.0 + self.strife_fraction())
        } else {
            base
        }
    }
}

impl Default for Strife {
    fn default() -> Self {
        Self::new(10.0, 1.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_empty() {
        let s = Strife::new(10.0, 1.0, 2.0);
        assert_eq!(s.strife, 0.0);
        assert!(!s.is_enraged());
    }

    #[test]
    fn hit_adds_gain_per_hit() {
        let mut s = Strife::new(10.0, 2.0, 0.0);
        s.hit();
        assert!((s.strife - 2.0).abs() < 1e-5);
    }

    #[test]
    fn hit_caps_at_max_strife() {
        let mut s = Strife::new(5.0, 3.0, 0.0);
        s.hit(); // 3
        s.hit(); // would be 6, caps at 5
        assert!((s.strife - 5.0).abs() < 1e-5);
    }

    #[test]
    fn hit_fires_just_peaked_on_transition() {
        let mut s = Strife::new(3.0, 1.0, 0.0);
        s.hit(); // 1
        s.hit(); // 2
        assert!(!s.just_peaked);
        s.hit(); // 3 = max → peaks
        assert!(s.just_peaked);
        assert!(s.is_enraged());
    }

    #[test]
    fn hit_no_just_peaked_when_already_enraged() {
        let mut s = Strife::new(2.0, 1.0, 0.0);
        s.hit(); // 1
        s.hit(); // 2 = peaks
        s.tick(0.0);
        s.hit(); // still at 2
        assert!(!s.just_peaked);
    }

    #[test]
    fn hit_no_op_when_disabled() {
        let mut s = Strife::new(10.0, 1.0, 0.0);
        s.enabled = false;
        s.hit();
        assert_eq!(s.strife, 0.0);
    }

    #[test]
    fn tick_decays_strife() {
        let mut s = Strife::new(10.0, 5.0, 1.0);
        s.hit(); // 5
        s.tick(2.0); // 5 - 2 = 3
        assert!((s.strife - 3.0).abs() < 1e-5);
    }

    #[test]
    fn tick_floors_at_zero() {
        let mut s = Strife::new(10.0, 2.0, 5.0);
        s.hit(); // 2
        s.tick(10.0); // would go negative
        assert_eq!(s.strife, 0.0);
    }

    #[test]
    fn tick_clears_just_peaked() {
        let mut s = Strife::new(1.0, 1.0, 0.0);
        s.hit(); // peaks
        s.tick(0.0);
        assert!(!s.just_peaked);
    }

    #[test]
    fn tick_no_decay_when_rate_zero() {
        let mut s = Strife::new(10.0, 5.0, 0.0);
        s.hit(); // 5
        s.tick(100.0);
        assert!((s.strife - 5.0).abs() < 1e-5);
    }

    #[test]
    fn is_enraged_false_when_disabled() {
        let mut s = Strife::new(2.0, 1.0, 0.0);
        s.hit();
        s.hit();
        s.enabled = false;
        assert!(!s.is_enraged());
    }

    #[test]
    fn strife_fraction_at_zero() {
        let s = Strife::new(10.0, 1.0, 0.0);
        assert_eq!(s.strife_fraction(), 0.0);
    }

    #[test]
    fn strife_fraction_at_half() {
        let mut s = Strife::new(10.0, 5.0, 0.0);
        s.hit(); // 5 out of 10
        assert!((s.strife_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn strife_fraction_at_full() {
        let mut s = Strife::new(4.0, 4.0, 0.0);
        s.hit(); // 4 = max
        assert!((s.strife_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn damage_bonus_scales_with_strife() {
        let mut s = Strife::new(10.0, 10.0, 0.0);
        s.hit(); // full strife
                 // 100 * (1 + 1.0) = 200
        assert!((s.damage_bonus(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn damage_bonus_at_half_strife() {
        let mut s = Strife::new(10.0, 5.0, 0.0);
        s.hit(); // 5 / 10 = 0.5 fraction
                 // 100 * (1 + 0.5) = 150
        assert!((s.damage_bonus(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn damage_bonus_base_when_empty() {
        let s = Strife::new(10.0, 1.0, 0.0);
        assert!((s.damage_bonus(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn damage_bonus_base_when_disabled() {
        let mut s = Strife::new(10.0, 10.0, 0.0);
        s.hit();
        s.enabled = false;
        assert!((s.damage_bonus(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn re_peaks_after_decay_and_hits() {
        let mut s = Strife::new(3.0, 1.0, 10.0);
        s.hit();
        s.hit();
        s.hit(); // peaks
        s.tick(0.0);
        s.tick(1.0); // decays to 0
        s.hit();
        s.hit();
        s.hit(); // peaks again
        assert!(s.just_peaked);
    }

    #[test]
    fn max_strife_clamped_to_one() {
        let s = Strife::new(0.0, 1.0, 0.0);
        assert!((s.max_strife - 1.0).abs() < 1e-5);
    }

    #[test]
    fn gain_per_hit_clamped_non_negative() {
        let s = Strife::new(10.0, -1.0, 0.0);
        assert_eq!(s.gain_per_hit, 0.0);
    }

    #[test]
    fn decay_rate_clamped_non_negative() {
        let s = Strife::new(10.0, 1.0, -5.0);
        assert_eq!(s.decay_rate, 0.0);
    }
}
