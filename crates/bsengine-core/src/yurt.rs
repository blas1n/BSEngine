use bevy_ecs::prelude::Component;

/// Portable deployment-state tracker. Models anything that must be set up
/// before it provides value and can be collapsed or destroyed. `deployed`
/// toggles via `deploy()` / `collapse()`. While deployed, `durability`
/// can be eroded by `damage(amount)`; reaching 0 auto-collapses.
///
/// Models field shelters, deployable turrets, pop-up cover, portable
/// generators, or any entity with an explicit set-up / tear-down lifecycle.
///
/// `deploy()` transitions to deployed when not already deployed and enabled.
/// Fires `just_deployed`. No-op when disabled.
///
/// `collapse()` tears down the shelter when deployed and enabled. Fires
/// `just_collapsed` and resets `durability` to `max_durability`. No-op
/// when disabled.
///
/// `damage(amount)` reduces durability while deployed. When durability
/// reaches 0, auto-collapses (fires `just_collapsed`). No-op when disabled
/// or not deployed.
///
/// `repair(amount)` restores durability while deployed, clamped to max.
/// No-op when disabled or not deployed.
///
/// `tick(_dt)` clears `just_deployed` and `just_collapsed`.
///
/// `is_deployed()` returns `deployed && enabled`.
///
/// `durability_fraction()` returns `(durability / max_durability).clamp(0,1)`.
///
/// `effective_protection(base)` returns `base * durability_fraction()` when
/// deployed and enabled; `0.0` otherwise. Models how shelter quality scales
/// with remaining durability.
///
/// Default: `new(100.0)` — collapsed, full durability.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yurt {
    pub durability: f32,
    pub max_durability: f32,
    pub deployed: bool,
    pub just_deployed: bool,
    pub just_collapsed: bool,
    pub enabled: bool,
}

impl Yurt {
    pub fn new(max_durability: f32) -> Self {
        let max = max_durability.max(0.1);
        Self {
            durability: max,
            max_durability: max,
            deployed: false,
            just_deployed: false,
            just_collapsed: false,
            enabled: true,
        }
    }

    /// Set up the shelter. Fires `just_deployed`. No-op when already deployed
    /// or disabled.
    pub fn deploy(&mut self) {
        if !self.enabled || self.deployed {
            return;
        }
        self.deployed = true;
        self.just_deployed = true;
    }

    /// Tear down the shelter. Fires `just_collapsed` and resets durability.
    /// No-op when not deployed or disabled.
    pub fn collapse(&mut self) {
        if !self.enabled || !self.deployed {
            return;
        }
        self.deployed = false;
        self.just_collapsed = true;
        self.durability = self.max_durability;
    }

    /// Reduce durability while deployed. Auto-collapses when durability
    /// reaches 0. No-op when disabled or not deployed.
    pub fn damage(&mut self, amount: f32) {
        if !self.enabled || !self.deployed || amount <= 0.0 {
            return;
        }
        self.durability = (self.durability - amount).max(0.0);
        if self.durability <= 0.0 {
            self.deployed = false;
            self.just_collapsed = true;
        }
    }

    /// Restore durability while deployed, clamped to `max_durability`.
    /// No-op when disabled or not deployed.
    pub fn repair(&mut self, amount: f32) {
        if !self.enabled || !self.deployed || amount <= 0.0 {
            return;
        }
        self.durability = (self.durability + amount).min(self.max_durability);
    }

    /// Advance one frame: clear `just_deployed` and `just_collapsed`.
    pub fn tick(&mut self, _dt: f32) {
        self.just_deployed = false;
        self.just_collapsed = false;
    }

    /// `true` when deployed and enabled.
    pub fn is_deployed(&self) -> bool {
        self.deployed && self.enabled
    }

    /// Fraction of maximum durability [0.0, 1.0].
    pub fn durability_fraction(&self) -> f32 {
        (self.durability / self.max_durability).clamp(0.0, 1.0)
    }

    /// Returns `base * durability_fraction()` when deployed and enabled;
    /// `0.0` otherwise.
    pub fn effective_protection(&self, base: f32) -> f32 {
        if !self.enabled || !self.deployed {
            return 0.0;
        }
        base * self.durability_fraction()
    }
}

impl Default for Yurt {
    fn default() -> Self {
        Self::new(100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yurt {
        Yurt::new(100.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_collapsed_with_full_durability() {
        let y = y();
        assert!(!y.deployed);
        assert_eq!(y.durability, 100.0);
        assert!(!y.is_deployed());
    }

    #[test]
    fn new_clamps_max_durability() {
        let y = Yurt::new(-5.0);
        assert!((y.max_durability - 0.1).abs() < 1e-5);
        assert!((y.durability - 0.1).abs() < 1e-5);
    }

    #[test]
    fn default_values() {
        let y = Yurt::default();
        assert!((y.max_durability - 100.0).abs() < 1e-5);
        assert!(!y.deployed);
    }

    // --- deploy ---

    #[test]
    fn deploy_sets_deployed() {
        let mut y = y();
        y.deploy();
        assert!(y.deployed);
        assert!(y.is_deployed());
    }

    #[test]
    fn deploy_fires_just_deployed() {
        let mut y = y();
        y.deploy();
        assert!(y.just_deployed);
    }

    #[test]
    fn deploy_no_op_when_already_deployed() {
        let mut y = y();
        y.deploy();
        y.tick(0.016);
        y.deploy(); // already deployed
        assert!(!y.just_deployed);
    }

    #[test]
    fn deploy_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.deploy();
        assert!(!y.deployed);
        assert!(!y.just_deployed);
    }

    // --- collapse ---

    #[test]
    fn collapse_clears_deployed() {
        let mut y = y();
        y.deploy();
        y.collapse();
        assert!(!y.deployed);
        assert!(!y.is_deployed());
    }

    #[test]
    fn collapse_fires_just_collapsed() {
        let mut y = y();
        y.deploy();
        y.collapse();
        assert!(y.just_collapsed);
    }

    #[test]
    fn collapse_resets_durability() {
        let mut y = y();
        y.deploy();
        y.damage(40.0);
        assert!((y.durability - 60.0).abs() < 1e-3);
        y.collapse();
        assert!((y.durability - 100.0).abs() < 1e-3);
    }

    #[test]
    fn collapse_no_op_when_not_deployed() {
        let mut y = y();
        y.collapse();
        assert!(!y.just_collapsed);
    }

    #[test]
    fn collapse_no_op_when_disabled() {
        let mut y = y();
        y.deploy();
        y.enabled = false;
        y.collapse();
        assert!(y.deployed); // still deployed
        assert!(!y.just_collapsed);
    }

    // --- damage ---

    #[test]
    fn damage_reduces_durability() {
        let mut y = y();
        y.deploy();
        y.damage(30.0);
        assert!((y.durability - 70.0).abs() < 1e-3);
    }

    #[test]
    fn damage_clamps_at_zero() {
        let mut y = y();
        y.deploy();
        y.damage(200.0);
        assert_eq!(y.durability, 0.0);
    }

    #[test]
    fn damage_auto_collapses_at_zero() {
        let mut y = y();
        y.deploy();
        y.damage(100.0);
        assert!(!y.deployed);
        assert!(y.just_collapsed);
    }

    #[test]
    fn damage_no_op_when_not_deployed() {
        let mut y = y();
        y.damage(50.0);
        assert!((y.durability - 100.0).abs() < 1e-3);
    }

    #[test]
    fn damage_no_op_when_disabled() {
        let mut y = y();
        y.deploy();
        y.enabled = false;
        y.damage(50.0);
        assert!((y.durability - 100.0).abs() < 1e-3);
    }

    #[test]
    fn damage_no_op_for_zero_amount() {
        let mut y = y();
        y.deploy();
        y.damage(0.0);
        assert!((y.durability - 100.0).abs() < 1e-3);
    }

    // --- repair ---

    #[test]
    fn repair_restores_durability() {
        let mut y = y();
        y.deploy();
        y.damage(40.0);
        y.repair(20.0);
        assert!((y.durability - 80.0).abs() < 1e-3);
    }

    #[test]
    fn repair_clamps_at_max() {
        let mut y = y();
        y.deploy();
        y.damage(10.0);
        y.repair(200.0);
        assert!((y.durability - 100.0).abs() < 1e-3);
    }

    #[test]
    fn repair_no_op_when_not_deployed() {
        let mut y = y();
        y.damage(10.0); // no-op, not deployed
        y.repair(50.0); // also no-op
        assert!((y.durability - 100.0).abs() < 1e-3);
    }

    #[test]
    fn repair_no_op_when_disabled() {
        let mut y = y();
        y.deploy();
        y.damage(30.0);
        y.enabled = false;
        y.repair(30.0);
        assert!((y.durability - 70.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_clears_just_deployed() {
        let mut y = y();
        y.deploy();
        y.tick(0.016);
        assert!(!y.just_deployed);
    }

    #[test]
    fn tick_clears_just_collapsed() {
        let mut y = y();
        y.deploy();
        y.collapse();
        y.tick(0.016);
        assert!(!y.just_collapsed);
    }

    #[test]
    fn tick_does_not_alter_state() {
        let mut y = y();
        y.deploy();
        y.damage(20.0);
        y.tick(1.0);
        assert!(y.deployed);
        assert!((y.durability - 80.0).abs() < 1e-3);
    }

    // --- is_deployed / fractions / effective ---

    #[test]
    fn is_deployed_false_when_disabled() {
        let mut y = y();
        y.deploy();
        y.enabled = false;
        assert!(!y.is_deployed());
    }

    #[test]
    fn durability_fraction_full_when_undamaged() {
        let mut y = y();
        y.deploy();
        assert!((y.durability_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn durability_fraction_half_at_midpoint() {
        let mut y = y();
        y.deploy();
        y.damage(50.0);
        assert!((y.durability_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_protection_zero_when_not_deployed() {
        let y = y();
        assert_eq!(y.effective_protection(100.0), 0.0);
    }

    #[test]
    fn effective_protection_full_when_undamaged() {
        let mut y = y();
        y.deploy();
        assert!((y.effective_protection(100.0) - 100.0).abs() < 1e-3);
    }

    #[test]
    fn effective_protection_scales_with_durability() {
        let mut y = y();
        y.deploy();
        y.damage(75.0);
        assert!((y.effective_protection(100.0) - 25.0).abs() < 1e-3);
    }

    #[test]
    fn effective_protection_zero_when_disabled() {
        let mut y = y();
        y.deploy();
        y.enabled = false;
        assert_eq!(y.effective_protection(100.0), 0.0);
    }
}
