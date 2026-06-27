use bevy_ecs::prelude::Component;

/// Ability-repetition mechanic: entity stores `charges` and the ability
/// system calls `consume()` just before it fires an effect — when a charge is
/// spent, `just_echoed` is set so the system can fire a second, weaker copy
/// of the same effect at `echo_fraction` power.
///
/// `load()` adds one charge up to `max_charges`. `consume()` spends a charge
/// and returns `true` when one was available; returns `false` when empty or
/// disabled. `tick()` clears `just_echoed` each frame.
///
/// `echo_power(base)` returns `base * echo_fraction` when a charge is ready
/// and enabled; 0.0 otherwise — use this to compute the secondary hit damage
/// or effect magnitude.
///
/// Distinct from `Reflect` (sends incoming attacks back), `Ricochet`
/// (projectile bouncing), and `Combo` (sequential hit escalation): Echo is
/// a **charge-based ability repetition** — the entity's next action
/// automatically fires a weaker duplicate sourced from the same entity.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Echo {
    pub charges: u32,
    /// Maximum echo charges. Clamped ≥ 1.
    pub max_charges: u32,
    /// Power fraction of the echoed effect [0.0, 1.0].
    pub echo_fraction: f32,
    pub just_echoed: bool,
    pub enabled: bool,
}

impl Echo {
    pub fn new(max_charges: u32, echo_fraction: f32) -> Self {
        Self {
            charges: 0,
            max_charges: max_charges.max(1),
            echo_fraction: echo_fraction.clamp(0.0, 1.0),
            just_echoed: false,
            enabled: true,
        }
    }

    /// Add one echo charge (capped at `max_charges`). No-op when disabled.
    pub fn load(&mut self) {
        if self.enabled && self.charges < self.max_charges {
            self.charges += 1;
        }
    }

    /// Spend one charge. Sets `just_echoed` and returns `true` when a charge
    /// was available and the component is enabled. Returns `false` otherwise.
    pub fn consume(&mut self) -> bool {
        if !self.enabled || self.charges == 0 {
            return false;
        }
        self.charges -= 1;
        self.just_echoed = true;
        true
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_echoed = false;
    }

    /// `true` when at least one charge is ready and the component is enabled.
    pub fn is_ready(&self) -> bool {
        self.charges > 0 && self.enabled
    }

    /// Power of the echoed effect. Returns `base * echo_fraction` when ready;
    /// 0.0 otherwise.
    pub fn echo_power(&self, base: f32) -> f32 {
        if self.is_ready() {
            base * self.echo_fraction
        } else {
            0.0
        }
    }

    /// Charge fill fraction [0.0 = empty, 1.0 = full].
    pub fn charge_fraction(&self) -> f32 {
        self.charges as f32 / self.max_charges as f32
    }
}

impl Default for Echo {
    fn default() -> Self {
        Self::new(1, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_empty() {
        let e = Echo::new(3, 0.5);
        assert_eq!(e.charges, 0);
        assert!(!e.is_ready());
    }

    #[test]
    fn load_adds_charge() {
        let mut e = Echo::new(3, 0.5);
        e.load();
        assert_eq!(e.charges, 1);
        assert!(e.is_ready());
    }

    #[test]
    fn load_caps_at_max_charges() {
        let mut e = Echo::new(2, 0.5);
        e.load();
        e.load();
        e.load(); // over cap
        assert_eq!(e.charges, 2);
    }

    #[test]
    fn load_no_op_when_disabled() {
        let mut e = Echo::new(3, 0.5);
        e.enabled = false;
        e.load();
        assert_eq!(e.charges, 0);
    }

    #[test]
    fn consume_spends_charge_and_returns_true() {
        let mut e = Echo::new(2, 0.5);
        e.load();
        assert!(e.consume());
        assert_eq!(e.charges, 0);
        assert!(e.just_echoed);
    }

    #[test]
    fn consume_returns_false_when_empty() {
        let mut e = Echo::new(1, 0.5);
        assert!(!e.consume());
        assert!(!e.just_echoed);
    }

    #[test]
    fn consume_returns_false_when_disabled() {
        let mut e = Echo::new(1, 0.5);
        e.load();
        e.enabled = false;
        assert!(!e.consume());
        assert_eq!(e.charges, 1); // not spent
    }

    #[test]
    fn consume_decrements_without_clearing_all() {
        let mut e = Echo::new(3, 0.5);
        e.load();
        e.load();
        e.load();
        e.consume();
        assert_eq!(e.charges, 2);
        assert!(e.is_ready());
    }

    #[test]
    fn tick_clears_just_echoed() {
        let mut e = Echo::new(1, 0.5);
        e.load();
        e.consume();
        e.tick();
        assert!(!e.just_echoed);
    }

    #[test]
    fn echo_power_scales_base() {
        let mut e = Echo::new(1, 0.5);
        e.load();
        // 100 * 0.5 = 50
        assert!((e.echo_power(100.0) - 50.0).abs() < 1e-5);
    }

    #[test]
    fn echo_power_zero_when_empty() {
        let e = Echo::new(1, 0.5);
        assert!((e.echo_power(100.0)).abs() < 1e-5);
    }

    #[test]
    fn echo_power_zero_when_disabled() {
        let mut e = Echo::new(1, 0.5);
        e.load();
        e.enabled = false;
        assert!((e.echo_power(100.0)).abs() < 1e-5);
    }

    #[test]
    fn charge_fraction_at_half() {
        let mut e = Echo::new(4, 0.5);
        e.load();
        e.load();
        assert!((e.charge_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn charge_fraction_zero_when_empty() {
        let e = Echo::new(1, 0.5);
        assert!((e.charge_fraction()).abs() < 1e-5);
    }

    #[test]
    fn max_charges_clamped_to_one() {
        let e = Echo::new(0, 0.5);
        assert_eq!(e.max_charges, 1);
    }

    #[test]
    fn echo_fraction_clamped_to_one() {
        let e = Echo::new(1, 2.0);
        assert!((e.echo_fraction - 1.0).abs() < 1e-5);
    }

    #[test]
    fn echo_fraction_clamped_at_zero() {
        let e = Echo::new(1, -0.5);
        assert!((e.echo_fraction).abs() < 1e-5);
    }

    #[test]
    fn can_reload_after_consuming() {
        let mut e = Echo::new(1, 0.5);
        e.load();
        e.consume();
        e.tick();
        e.load();
        assert!(e.is_ready());
    }
}
