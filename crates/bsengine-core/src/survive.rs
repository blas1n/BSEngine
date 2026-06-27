use bevy_ecs::prelude::Component;

/// Killing-blow negation token: when HP would drop to 0 or below, the death
/// system calls `consume()` — it spends one charge and returns `true`
/// (entity survives with 1 HP), setting `just_survived`. Returns `false`
/// (entity dies normally) when no charges remain. `tick()` clears
/// one-frame flags.
///
/// Charges are loaded via `add_charge()` (capped at `max_charges`).
///
/// Distinct from `Invincible` (full damage immunity for a timed window),
/// `Revive` (entity can get back up after dying), and `Barrier` (absorbs
/// damage before it reaches HP): Survive is a **killing-blow negation
/// token** — it does nothing until the moment of death, then consumes one
/// charge to prevent it. Bypasses healing systems entirely; the death system
/// is solely responsible for calling `consume()`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Survive {
    pub charges: u32,
    pub max_charges: u32,
    pub just_survived: bool,
    pub enabled: bool,
}

impl Survive {
    pub fn new(max_charges: u32) -> Self {
        Self {
            charges: 0,
            max_charges: max_charges.max(1),
            just_survived: false,
            enabled: true,
        }
    }

    /// Load one charge (capped at `max_charges`). No-op when disabled.
    pub fn add_charge(&mut self) {
        if self.enabled && self.charges < self.max_charges {
            self.charges += 1;
        }
    }

    /// Called by the death system when HP hits 0. Spends one charge and
    /// returns `true` (entity survives) while charges remain. Returns `false`
    /// when empty or disabled.
    pub fn consume(&mut self) -> bool {
        if !self.enabled || self.charges == 0 {
            return false;
        }
        self.charges -= 1;
        self.just_survived = true;
        true
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_survived = false;
    }

    pub fn is_ready(&self) -> bool {
        self.charges > 0 && self.enabled
    }

    /// Fraction of max charges available [0.0 = empty, 1.0 = full].
    pub fn charge_fraction(&self) -> f32 {
        if self.max_charges == 0 {
            return 0.0;
        }
        (self.charges as f32 / self.max_charges as f32).clamp(0.0, 1.0)
    }
}

impl Default for Survive {
    fn default() -> Self {
        Self::new(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_charge_increments() {
        let mut s = Survive::new(3);
        s.add_charge();
        assert_eq!(s.charges, 1);
        assert!(s.is_ready());
    }

    #[test]
    fn add_charge_caps_at_max() {
        let mut s = Survive::new(2);
        s.add_charge();
        s.add_charge();
        s.add_charge(); // over cap
        assert_eq!(s.charges, 2);
    }

    #[test]
    fn consume_returns_true_and_spends_charge() {
        let mut s = Survive::new(1);
        s.add_charge();
        assert!(s.consume());
        assert_eq!(s.charges, 0);
        assert!(s.just_survived);
    }

    #[test]
    fn consume_returns_false_when_empty() {
        let mut s = Survive::new(1);
        assert!(!s.consume());
        assert!(!s.just_survived);
    }

    #[test]
    fn consume_decrements_without_clearing_all() {
        let mut s = Survive::new(3);
        s.add_charge();
        s.add_charge();
        s.add_charge();
        s.consume();
        assert_eq!(s.charges, 2);
        assert!(s.is_ready());
    }

    #[test]
    fn tick_clears_just_survived() {
        let mut s = Survive::new(1);
        s.add_charge();
        s.consume();
        s.tick();
        assert!(!s.just_survived);
    }

    #[test]
    fn is_ready_false_when_empty() {
        let s = Survive::new(1);
        assert!(!s.is_ready());
    }

    #[test]
    fn is_ready_false_when_disabled() {
        let mut s = Survive::new(1);
        s.add_charge();
        s.enabled = false;
        assert!(!s.is_ready());
    }

    #[test]
    fn charge_fraction_at_half() {
        let mut s = Survive::new(4);
        s.add_charge();
        s.add_charge();
        assert!((s.charge_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn disabled_add_charge_no_op() {
        let mut s = Survive::new(3);
        s.enabled = false;
        s.add_charge();
        assert_eq!(s.charges, 0);
    }

    #[test]
    fn disabled_consume_returns_false() {
        let mut s = Survive::new(3);
        s.add_charge();
        s.enabled = false;
        assert!(!s.consume());
        assert_eq!(s.charges, 1); // charge not spent
    }
}
