use bevy_ecs::prelude::Component;

/// Acid debuff that erodes the entity's armor. Each stack of corrosion reduces
/// effective armor by `armor_reduction_per_stack`. Stacks decay at
/// `decay_rate` per second when not refreshed.
///
/// The armor pipeline reads `armor_reduction()` and subtracts it from the
/// entity's base armor value (floored at zero) before applying damage.
///
/// `apply(amount)` adds stacks up to `max_stacks`, setting `just_corroded` the
/// first time stacks become positive. `tick(dt)` decays stacks and sets
/// `just_cleared` when they reach zero via natural decay.
///
/// Distinct from `Weaken` (outgoing damage penalty) and `Expose` (flat
/// defense-reduction debuff applied from outside): Corrosion is self-
/// accumulating acid damage — the more acid that is applied, the deeper the
/// armor erosion, and it fades on its own when the source stops.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Corrosion {
    /// Current corrosion intensity.
    pub stacks: f32,
    /// Maximum corrosion stacks (acid saturation cap).
    pub max_stacks: f32,
    /// Stacks lost per second (acid neutralization).
    pub decay_rate: f32,
    /// Flat armor reduction per stack while corroded.
    pub armor_reduction_per_stack: f32,
    pub just_corroded: bool,
    pub just_cleared: bool,
    pub enabled: bool,
}

impl Corrosion {
    pub fn new(armor_reduction_per_stack: f32, max_stacks: f32) -> Self {
        Self {
            stacks: 0.0,
            max_stacks: max_stacks.max(0.0),
            decay_rate: 1.0,
            armor_reduction_per_stack: armor_reduction_per_stack.max(0.0),
            just_corroded: false,
            just_cleared: false,
            enabled: true,
        }
    }

    pub fn with_decay(mut self, decay_rate: f32) -> Self {
        self.decay_rate = decay_rate.max(0.0);
        self
    }

    /// Add `amount` corrosion stacks, capped at `max_stacks`. No-op when
    /// disabled. Sets `just_corroded` the first time stacks go positive.
    pub fn apply(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_empty = !self.has_corrosion();
        self.stacks = (self.stacks + amount).min(self.max_stacks);
        if was_empty && self.has_corrosion() {
            self.just_corroded = true;
        }
    }

    /// Decay stacks by `decay_rate * dt`. Sets `just_cleared` when stacks
    /// reach zero via natural decay. Returns the current stack count after decay.
    pub fn tick(&mut self, dt: f32) -> f32 {
        self.just_corroded = false;
        self.just_cleared = false;

        if self.stacks > 0.0 {
            self.stacks -= self.decay_rate * dt;
            if self.stacks <= 0.0 {
                self.stacks = 0.0;
                self.just_cleared = true;
            }
        }

        self.stacks
    }

    pub fn has_corrosion(&self) -> bool {
        self.stacks > 0.0
    }

    /// Total flat armor reduction from current corrosion stacks.
    pub fn armor_reduction(&self) -> f32 {
        self.stacks * self.armor_reduction_per_stack
    }

    /// Progress toward `max_stacks` [0.0, 1.0].
    pub fn stack_fraction(&self) -> f32 {
        if self.max_stacks <= 0.0 {
            return 0.0;
        }
        (self.stacks / self.max_stacks).clamp(0.0, 1.0)
    }
}

impl Default for Corrosion {
    fn default() -> Self {
        Self::new(5.0, 10.0).with_decay(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_adds_stacks() {
        let mut c = Corrosion::new(5.0, 10.0);
        c.apply(3.0);
        assert!(c.has_corrosion());
        assert!(c.just_corroded);
        assert!((c.stacks - 3.0).abs() < 1e-5);
    }

    #[test]
    fn apply_capped_at_max() {
        let mut c = Corrosion::new(5.0, 10.0);
        c.apply(15.0);
        assert!((c.stacks - 10.0).abs() < 1e-5);
    }

    #[test]
    fn just_corroded_only_on_first_apply() {
        let mut c = Corrosion::new(5.0, 10.0);
        c.apply(2.0);
        c.tick(0.016);
        c.apply(1.0);
        assert!(!c.just_corroded);
    }

    #[test]
    fn apply_no_op_when_disabled() {
        let mut c = Corrosion::new(5.0, 10.0);
        c.enabled = false;
        c.apply(5.0);
        assert!(!c.has_corrosion());
    }

    #[test]
    fn tick_decays_stacks() {
        let mut c = Corrosion::new(5.0, 10.0).with_decay(2.0);
        c.apply(4.0);
        c.tick(1.0);
        assert!((c.stacks - 2.0).abs() < 1e-4);
    }

    #[test]
    fn tick_just_cleared_on_full_decay() {
        let mut c = Corrosion::new(5.0, 10.0).with_decay(5.0);
        c.apply(3.0);
        c.tick(1.0);
        assert!(!c.has_corrosion());
        assert!(c.just_cleared);
    }

    #[test]
    fn armor_reduction_scales_with_stacks() {
        let mut c = Corrosion::new(3.0, 10.0);
        c.apply(4.0);
        assert!((c.armor_reduction() - 12.0).abs() < 1e-4); // 4 * 3
    }

    #[test]
    fn armor_reduction_zero_when_no_corrosion() {
        let c = Corrosion::new(3.0, 10.0);
        assert!((c.armor_reduction() - 0.0).abs() < 1e-5);
    }

    #[test]
    fn stack_fraction_at_half() {
        let mut c = Corrosion::new(5.0, 10.0);
        c.apply(5.0);
        assert!((c.stack_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_just_corroded() {
        let mut c = Corrosion::new(5.0, 10.0);
        c.apply(3.0);
        c.tick(0.016);
        assert!(!c.just_corroded);
    }
}
