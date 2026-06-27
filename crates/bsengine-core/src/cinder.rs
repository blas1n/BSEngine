use bevy_ecs::prelude::Component;

/// Smoldering ember residue left by fire. Embers decay over time, dealing low
/// ongoing heat damage. When an ignition source triggers `try_reignite()` and
/// stacks have reached `reignite_threshold`, a burst of `burst_damage` is
/// released and the embers are consumed.
///
/// `kindle(amount)` adds ember stacks. `tick(dt)` decays stacks at
/// `decay_rate` per second and returns `stacks * damage_per_stack_per_second *
/// dt` as the smolder pulse. `try_reignite()` tests the threshold; if met,
/// clears stacks and returns `burst_damage`, otherwise returns `0.0`.
///
/// Set `reignite_threshold` to `0.0` (default) to disable burst reignition —
/// the component still tracks and decays embers and deals passive damage.
///
/// Distinct from `Burn` (fixed-duration DoT) and `Blaze` (outward AoE aura):
/// Cinder lingers as persistent embers that decay and can explode on reignition.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Cinder {
    /// Current ember intensity.
    pub stacks: f32,
    /// Ember stacks lost per second (natural cooling).
    pub decay_rate: f32,
    /// Passive heat damage per stack per second while smoldering.
    pub damage_per_stack_per_second: f32,
    /// Stack threshold to trigger a burst on `try_reignite()`. 0.0 = disabled.
    pub reignite_threshold: f32,
    /// Damage returned by a successful `try_reignite()`.
    pub burst_damage: f32,
    pub just_kindled: bool,
    pub just_reignited: bool,
    pub just_burned_out: bool,
    pub enabled: bool,
}

impl Cinder {
    pub fn new(damage_per_stack_per_second: f32, decay_rate: f32) -> Self {
        Self {
            stacks: 0.0,
            decay_rate: decay_rate.max(0.0),
            damage_per_stack_per_second: damage_per_stack_per_second.max(0.0),
            reignite_threshold: 0.0,
            burst_damage: 0.0,
            just_kindled: false,
            just_reignited: false,
            just_burned_out: false,
            enabled: true,
        }
    }

    pub fn with_reignite(mut self, threshold: f32, burst_damage: f32) -> Self {
        self.reignite_threshold = threshold.max(0.0);
        self.burst_damage = burst_damage.max(0.0);
        self
    }

    /// Add ember stacks. No-op when disabled.
    pub fn kindle(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_empty = !self.has_embers();
        self.stacks += amount;
        if was_empty {
            self.just_kindled = true;
        }
    }

    /// If `reignite_threshold > 0` and `stacks >= threshold`, consumes all
    /// stacks, sets `just_reignited`, and returns `burst_damage`. Otherwise
    /// returns `0.0`.
    pub fn try_reignite(&mut self) -> f32 {
        if self.reignite_threshold > 0.0 && self.stacks >= self.reignite_threshold {
            self.stacks = 0.0;
            self.just_reignited = true;
            self.burst_damage
        } else {
            0.0
        }
    }

    /// Decay embers and return the passive smolder damage pulse for this frame.
    /// Sets `just_burned_out` when stacks reach zero via decay.
    pub fn tick(&mut self, dt: f32) -> f32 {
        self.just_kindled = false;
        self.just_reignited = false;
        self.just_burned_out = false;

        if self.stacks <= 0.0 {
            return 0.0;
        }

        let damage = self.stacks * self.damage_per_stack_per_second * dt;

        self.stacks -= self.decay_rate * dt;
        if self.stacks <= 0.0 {
            self.stacks = 0.0;
            self.just_burned_out = true;
        }

        damage
    }

    pub fn has_embers(&self) -> bool {
        self.stacks > 0.0
    }

    /// Progress toward reignite threshold [0.0, 1.0]. Returns 0.0 when
    /// threshold is disabled or stacks are empty.
    pub fn stack_fraction(&self) -> f32 {
        if self.reignite_threshold <= 0.0 {
            return 0.0;
        }
        (self.stacks / self.reignite_threshold).clamp(0.0, 1.0)
    }
}

impl Default for Cinder {
    fn default() -> Self {
        Self::new(2.0, 1.0).with_reignite(10.0, 30.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kindle_adds_stacks() {
        let mut c = Cinder::new(2.0, 1.0);
        c.kindle(5.0);
        assert!(c.has_embers());
        assert!(c.just_kindled);
        assert!((c.stacks - 5.0).abs() < 1e-5);
    }

    #[test]
    fn kindle_no_op_when_disabled() {
        let mut c = Cinder::new(2.0, 1.0);
        c.enabled = false;
        c.kindle(5.0);
        assert!(!c.has_embers());
    }

    #[test]
    fn just_kindled_only_on_first_stack() {
        let mut c = Cinder::new(2.0, 1.0);
        c.kindle(3.0);
        c.kindle(3.0);
        assert!(c.just_kindled); // set on first kindle this turn
        c.tick(0.016);
        c.kindle(1.0); // embers already present — not just_kindled
        assert!(!c.just_kindled);
    }

    #[test]
    fn tick_returns_smolder_damage() {
        let mut c = Cinder::new(2.0, 0.0); // no decay
        c.kindle(5.0);
        let dmg = c.tick(1.0);
        assert!((dmg - 10.0).abs() < 1e-4); // 5 * 2 * 1
    }

    #[test]
    fn tick_decays_stacks() {
        let mut c = Cinder::new(0.0, 2.0);
        c.kindle(4.0);
        c.tick(1.0);
        assert!((c.stacks - 2.0).abs() < 1e-4);
    }

    #[test]
    fn tick_just_burned_out_on_full_decay() {
        let mut c = Cinder::new(0.0, 5.0);
        c.kindle(3.0);
        c.tick(1.0);
        assert!(!c.has_embers());
        assert!(c.just_burned_out);
    }

    #[test]
    fn try_reignite_triggers_burst() {
        let mut c = Cinder::new(2.0, 1.0).with_reignite(5.0, 40.0);
        c.kindle(5.0);
        let burst = c.try_reignite();
        assert!((burst - 40.0).abs() < 1e-5);
        assert!(c.just_reignited);
        assert!(!c.has_embers());
    }

    #[test]
    fn try_reignite_no_trigger_below_threshold() {
        let mut c = Cinder::new(2.0, 1.0).with_reignite(10.0, 40.0);
        c.kindle(4.0);
        let burst = c.try_reignite();
        assert!((burst - 0.0).abs() < 1e-5);
        assert!(!c.just_reignited);
    }

    #[test]
    fn try_reignite_disabled_when_threshold_zero() {
        let mut c = Cinder::new(2.0, 1.0); // threshold = 0
        c.kindle(100.0);
        let burst = c.try_reignite();
        assert!((burst - 0.0).abs() < 1e-5);
    }

    #[test]
    fn stack_fraction_at_half() {
        let mut c = Cinder::new(2.0, 0.0).with_reignite(10.0, 30.0);
        c.kindle(5.0);
        assert!((c.stack_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_just_kindled() {
        let mut c = Cinder::new(2.0, 0.0);
        c.kindle(3.0);
        c.tick(0.016);
        assert!(!c.just_kindled);
    }
}
