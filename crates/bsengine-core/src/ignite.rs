use bevy_ecs::prelude::Component;

/// Flammable-charge accumulator that tracks ignite stacks and fires
/// `just_ignited` when they cross the threshold, signalling the combat system
/// to apply a full `Burn`.
///
/// Fire-dealing hits call `add_stacks(amount)`. Stacks decay at
/// `decay_rate` per second between hits. When stacks fall back to zero from
/// an ignited state, `just_extinguished` is set and `is_ignited()` returns
/// false. `extinguish()` clears stacks immediately (e.g. from water dousing).
///
/// Distinct from `Burn` (active damage-over-time): Ignite is the charge
/// meter that leads to Burn. A design that skips stacks and applies Burn
/// directly does not need this component.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Ignite {
    /// Current ignite stacks accumulated.
    pub stacks: f32,
    /// Stacks required to enter the ignited (burning) state.
    pub threshold: f32,
    /// Stacks lost per second while no new stacks are being added.
    pub decay_rate: f32,
    pub just_ignited: bool,
    pub just_extinguished: bool,
    pub enabled: bool,
}

impl Ignite {
    pub fn new(threshold: f32) -> Self {
        Self {
            stacks: 0.0,
            threshold: threshold.max(0.0),
            decay_rate: 0.0,
            just_ignited: false,
            just_extinguished: false,
            enabled: true,
        }
    }

    pub fn with_decay_rate(mut self, rate: f32) -> Self {
        self.decay_rate = rate.max(0.0);
        self
    }

    /// Add ignite stacks. Sets `just_ignited` when crossing the threshold
    /// for the first time this tick. No-op when disabled.
    pub fn add_stacks(&mut self, amount: f32) {
        if !self.enabled {
            return;
        }

        let was_ignited = self.is_ignited();
        self.stacks = (self.stacks + amount.max(0.0)).max(0.0);
        if !was_ignited && self.is_ignited() {
            self.just_ignited = true;
        }
    }

    /// Remove stacks immediately (e.g. doused by water). Sets
    /// `just_extinguished` when transitioning out of the ignited state.
    pub fn remove_stacks(&mut self, amount: f32) {
        let was_ignited = self.is_ignited();
        self.stacks = (self.stacks - amount.max(0.0)).max(0.0);
        if was_ignited && !self.is_ignited() {
            self.just_extinguished = true;
        }
    }

    /// Clear all stacks immediately.
    pub fn extinguish(&mut self) {
        if self.stacks > 0.0 {
            let was_ignited = self.is_ignited();
            self.stacks = 0.0;
            if was_ignited {
                self.just_extinguished = true;
            }
        }
    }

    /// Advance decay; sets `just_extinguished` when stacks reach zero from
    /// an ignited state. Clears per-frame pulse flags at start of tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_ignited = false;
        self.just_extinguished = false;

        if self.stacks > 0.0 && self.decay_rate > 0.0 {
            let was_ignited = self.is_ignited();
            self.stacks = (self.stacks - self.decay_rate * dt).max(0.0);
            if was_ignited && !self.is_ignited() {
                self.just_extinguished = true;
            }
        }
    }

    pub fn is_ignited(&self) -> bool {
        self.stacks >= self.threshold && self.threshold > 0.0
    }

    /// Fraction of the threshold filled [0.0 = empty, 1.0+ = ignited].
    pub fn stack_fraction(&self) -> f32 {
        if self.threshold <= 0.0 {
            return 0.0;
        }
        self.stacks / self.threshold
    }
}

impl Default for Ignite {
    fn default() -> Self {
        Self::new(100.0).with_decay_rate(20.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_stacks_below_threshold_not_ignited() {
        let mut ig = Ignite::new(100.0);
        ig.add_stacks(50.0);
        assert!(!ig.is_ignited());
        assert!(!ig.just_ignited);
    }

    #[test]
    fn add_stacks_crosses_threshold() {
        let mut ig = Ignite::new(100.0);
        ig.add_stacks(100.0);
        assert!(ig.is_ignited());
        assert!(ig.just_ignited);
    }

    #[test]
    fn add_stacks_no_double_just_ignited() {
        let mut ig = Ignite::new(100.0);
        ig.add_stacks(100.0);
        ig.tick(0.016);
        ig.add_stacks(10.0);
        assert!(!ig.just_ignited); // already ignited, no re-fire
    }

    #[test]
    fn remove_stacks_drops_below_threshold() {
        let mut ig = Ignite::new(100.0);
        ig.add_stacks(100.0);
        ig.tick(0.0); // clear just_ignited
        ig.remove_stacks(50.0);
        assert!(!ig.is_ignited());
        assert!(ig.just_extinguished);
    }

    #[test]
    fn extinguish_clears_all_stacks() {
        let mut ig = Ignite::new(100.0);
        ig.add_stacks(100.0);
        ig.tick(0.0);
        ig.extinguish();
        assert_eq!(ig.stacks, 0.0);
        assert!(!ig.is_ignited());
        assert!(ig.just_extinguished);
    }

    #[test]
    fn tick_decays_stacks() {
        let mut ig = Ignite::new(100.0).with_decay_rate(10.0);
        ig.add_stacks(100.0);
        ig.tick(1.0);
        assert!((ig.stacks - 90.0).abs() < 1e-4);
    }

    #[test]
    fn tick_extinguishes_on_decay_below_threshold() {
        let mut ig = Ignite::new(100.0).with_decay_rate(100.0);
        ig.add_stacks(100.0);
        ig.tick(0.0); // clear just_ignited
        ig.tick(1.1); // decay past threshold
        assert!(!ig.is_ignited());
        assert!(ig.just_extinguished);
    }

    #[test]
    fn stack_fraction_at_half() {
        let mut ig = Ignite::new(100.0);
        ig.add_stacks(50.0);
        assert!((ig.stack_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn disabled_add_stacks_no_op() {
        let mut ig = Ignite::new(100.0);
        ig.enabled = false;
        ig.add_stacks(100.0);
        assert_eq!(ig.stacks, 0.0);
    }

    #[test]
    fn tick_clears_just_ignited() {
        let mut ig = Ignite::new(100.0);
        ig.add_stacks(100.0);
        ig.tick(0.016);
        assert!(!ig.just_ignited);
    }
}
