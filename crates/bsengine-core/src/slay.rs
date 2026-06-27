use bevy_ecs::prelude::Component;

/// Threshold kill-counter reward trigger: entity accumulates kills via
/// `register_kill()`. Each time `kill_count` crosses a multiple of
/// `threshold`, `just_triggered` is set for one frame, `trigger_count`
/// increments, and `kill_count` resets to 0.
///
/// The reward system listens for `just_triggered` and applies the
/// appropriate bonus (e.g., healing, buff activation) independently.
/// `tick()` clears one-frame flags.
///
/// Distinct from `Fervor` (kill-streak speed/damage that passively
/// decays), `Rampage` (escalating berserk triggered on rapid kills), and
/// `Combo` (action-chaining multiplier): Slay is a **milestone kill-counter**
/// — it rewards consistent killing at fixed intervals without any time
/// pressure or decay, building a permanently increasing trigger total.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Slay {
    /// Kills accumulated since the last trigger.
    pub kill_count: u32,
    /// Number of kills required to fire a trigger. Clamped ≥ 1.
    pub threshold: u32,
    /// Total triggers fired over the entity's lifetime.
    pub trigger_count: u32,
    pub just_triggered: bool,
    pub enabled: bool,
}

impl Slay {
    pub fn new(threshold: u32) -> Self {
        Self {
            kill_count: 0,
            threshold: threshold.max(1),
            trigger_count: 0,
            just_triggered: false,
            enabled: true,
        }
    }

    /// Register one kill. Increments `kill_count`; if it reaches `threshold`,
    /// fires `just_triggered`, increments `trigger_count`, and resets
    /// `kill_count` to 0. No-op when disabled.
    pub fn register_kill(&mut self) {
        if !self.enabled {
            return;
        }
        self.kill_count += 1;
        if self.kill_count >= self.threshold {
            self.kill_count = 0;
            self.trigger_count += 1;
            self.just_triggered = true;
        }
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_triggered = false;
    }

    /// Progress towards the next trigger as a fraction [0.0 = just triggered, 1.0 = threshold reached].
    pub fn progress_fraction(&self) -> f32 {
        if self.threshold == 0 {
            return 0.0;
        }
        (self.kill_count as f32 / self.threshold as f32).clamp(0.0, 1.0)
    }

    /// Returns `true` when at least one trigger has fired.
    pub fn has_triggered(&self) -> bool {
        self.trigger_count > 0
    }
}

impl Default for Slay {
    fn default() -> Self {
        Self::new(5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_kill_increments_count() {
        let mut s = Slay::new(5);
        s.register_kill();
        assert_eq!(s.kill_count, 1);
        assert!(!s.just_triggered);
    }

    #[test]
    fn trigger_fires_at_threshold() {
        let mut s = Slay::new(3);
        s.register_kill();
        s.register_kill();
        s.register_kill();
        assert!(s.just_triggered);
        assert_eq!(s.trigger_count, 1);
        assert_eq!(s.kill_count, 0);
    }

    #[test]
    fn kill_count_resets_after_trigger() {
        let mut s = Slay::new(2);
        s.register_kill();
        s.register_kill();
        assert_eq!(s.kill_count, 0);
        s.tick();
        s.register_kill();
        assert_eq!(s.kill_count, 1);
        assert!(!s.just_triggered);
    }

    #[test]
    fn multiple_triggers_accumulate() {
        let mut s = Slay::new(1);
        s.register_kill();
        s.tick();
        s.register_kill();
        s.tick();
        s.register_kill();
        assert_eq!(s.trigger_count, 3);
    }

    #[test]
    fn tick_clears_just_triggered() {
        let mut s = Slay::new(1);
        s.register_kill();
        s.tick();
        assert!(!s.just_triggered);
    }

    #[test]
    fn progress_fraction_partial() {
        let mut s = Slay::new(4);
        s.register_kill();
        s.register_kill();
        assert!((s.progress_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn progress_fraction_zero_after_trigger() {
        let mut s = Slay::new(2);
        s.register_kill();
        s.register_kill();
        assert!((s.progress_fraction()).abs() < 1e-5);
    }

    #[test]
    fn has_triggered_false_initially() {
        let s = Slay::new(5);
        assert!(!s.has_triggered());
    }

    #[test]
    fn has_triggered_true_after_first_trigger() {
        let mut s = Slay::new(1);
        s.register_kill();
        assert!(s.has_triggered());
    }

    #[test]
    fn disabled_register_kill_no_op() {
        let mut s = Slay::new(1);
        s.enabled = false;
        s.register_kill();
        assert_eq!(s.kill_count, 0);
        assert!(!s.just_triggered);
    }

    #[test]
    fn threshold_one_triggers_every_kill() {
        let mut s = Slay::new(1);
        for i in 1..=5 {
            s.register_kill();
            assert!(s.just_triggered);
            assert_eq!(s.trigger_count, i);
            s.tick();
        }
    }

    #[test]
    fn default_threshold_five() {
        let s = Slay::default();
        assert_eq!(s.threshold, 5);
    }

    #[test]
    fn threshold_zero_clamped_to_one() {
        let s = Slay::new(0);
        assert_eq!(s.threshold, 1);
    }
}
