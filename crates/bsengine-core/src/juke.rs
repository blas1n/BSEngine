use bevy_ecs::prelude::Component;

/// Single-use evasion token: charged misdirection maneuvers that cause the
/// next incoming hit to miss.
///
/// Call `prime()` to add a charge (up to `max_charges`). When an attack check
/// lands, the hit system calls `consume()` — it spends one charge and returns
/// `true` (signalling a miss), setting `just_juked`. Returns `false` when no
/// charges remain (the hit lands normally). `tick()` clears one-frame flags.
///
/// Distinct from `Dodge` (timed invulnerability window), `Phase`
/// (teleportation through), `Deflect` (sends the attack back), and `Evade`
/// (probability-based miss chance): Juke is a **charged directional bait** —
/// stored maneuver tokens consumed on demand by the hit pipeline, with no
/// time window. Charges can be pre-loaded (e.g., while sprinting) and spent
/// reactively.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Juke {
    pub charges: u32,
    pub max_charges: u32,
    pub just_juked: bool,
    pub enabled: bool,
}

impl Juke {
    pub fn new(max_charges: u32) -> Self {
        Self {
            charges: 0,
            max_charges: max_charges.max(1),
            just_juked: false,
            enabled: true,
        }
    }

    /// Add one charge (capped at `max_charges`). No-op when disabled.
    pub fn prime(&mut self) {
        if self.enabled && self.charges < self.max_charges {
            self.charges += 1;
        }
    }

    /// Spend one charge. Returns `true` (miss) and sets `just_juked` if
    /// charges remain; returns `false` (hit lands) otherwise. No-op (returns
    /// `false`) when disabled.
    pub fn consume(&mut self) -> bool {
        if !self.enabled || self.charges == 0 {
            return false;
        }
        self.charges -= 1;
        self.just_juked = true;
        true
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_juked = false;
    }

    pub fn is_ready(&self) -> bool {
        self.charges > 0 && self.enabled
    }

    /// Fraction of max charges loaded [0.0 = empty, 1.0 = full].
    pub fn charge_fraction(&self) -> f32 {
        if self.max_charges == 0 {
            return 0.0;
        }
        (self.charges as f32 / self.max_charges as f32).clamp(0.0, 1.0)
    }
}

impl Default for Juke {
    fn default() -> Self {
        Self::new(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prime_adds_charge() {
        let mut j = Juke::new(3);
        j.prime();
        assert_eq!(j.charges, 1);
        assert!(j.is_ready());
    }

    #[test]
    fn prime_caps_at_max() {
        let mut j = Juke::new(2);
        j.prime();
        j.prime();
        j.prime(); // over cap
        assert_eq!(j.charges, 2);
    }

    #[test]
    fn consume_returns_true_and_spends_charge() {
        let mut j = Juke::new(1);
        j.prime();
        assert!(j.consume());
        assert_eq!(j.charges, 0);
        assert!(j.just_juked);
    }

    #[test]
    fn consume_returns_false_when_empty() {
        let mut j = Juke::new(1);
        assert!(!j.consume());
        assert!(!j.just_juked);
    }

    #[test]
    fn consume_decrements_without_clearing_all() {
        let mut j = Juke::new(3);
        j.prime();
        j.prime();
        j.prime();
        j.consume();
        assert_eq!(j.charges, 2);
        assert!(j.is_ready());
    }

    #[test]
    fn tick_clears_just_juked() {
        let mut j = Juke::new(1);
        j.prime();
        j.consume();
        j.tick();
        assert!(!j.just_juked);
    }

    #[test]
    fn charge_fraction_at_half() {
        let mut j = Juke::new(4);
        j.prime();
        j.prime();
        assert!((j.charge_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn charge_fraction_zero_when_empty() {
        let j = Juke::new(3);
        assert!((j.charge_fraction() - 0.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_prime_no_op() {
        let mut j = Juke::new(3);
        j.enabled = false;
        j.prime();
        assert_eq!(j.charges, 0);
    }

    #[test]
    fn disabled_consume_returns_false() {
        let mut j = Juke::new(3);
        j.prime();
        j.enabled = false;
        assert!(!j.consume());
        assert_eq!(j.charges, 1); // charge not spent
    }

    #[test]
    fn is_ready_false_when_empty() {
        let j = Juke::new(1);
        assert!(!j.is_ready());
    }

    #[test]
    fn is_ready_false_when_disabled() {
        let mut j = Juke::new(1);
        j.prime();
        j.enabled = false;
        assert!(!j.is_ready());
    }
}
