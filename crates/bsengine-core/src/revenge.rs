use bevy_ecs::prelude::Component;

/// Low-health power spike: when HP first drops below a threshold, grants a
/// temporary outgoing-damage multiplier for `duration` seconds.
///
/// Call `check_and_trigger(current_hp, max_hp)` each frame; it returns `true`
/// and sets `just_triggered` the frame the spike fires. The effect runs for
/// `duration` seconds (`tick(dt)` expires it, setting `just_ended`). Once
/// expired the `triggered` latch stays set — call `reset()` to allow the
/// spike to fire again (typically when HP rises back above the threshold).
///
/// While `is_active()`, `effective_damage(base)` returns
/// `base * revenge_multiplier`. Outside the window it returns `base`.
///
/// Distinct from `Rage` (builds from sustained hits over time), `Fervor`
/// (speed/haste spike on kill), and `Retaliate` (counterattack on taking
/// damage): Revenge is a **low-health one-shot power spike** — fires once per
/// reset when HP crosses the threshold from above, not from repeated events.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Revenge {
    pub duration: f32,
    pub timer: f32,
    /// Outgoing damage multiplier while the spike is active. Clamped ≥ 1.0.
    pub revenge_multiplier: f32,
    /// Fraction of max HP below which the spike triggers. Clamped to [0.0, 1.0].
    pub trigger_fraction: f32,
    /// True once the spike has fired and not yet been reset.
    pub triggered: bool,
    pub just_triggered: bool,
    pub just_ended: bool,
    pub enabled: bool,
}

impl Revenge {
    pub fn new(revenge_multiplier: f32, trigger_fraction: f32, duration: f32) -> Self {
        Self {
            duration: duration.max(0.0),
            timer: 0.0,
            revenge_multiplier: revenge_multiplier.max(1.0),
            trigger_fraction: trigger_fraction.clamp(0.0, 1.0),
            triggered: false,
            just_triggered: false,
            just_ended: false,
            enabled: true,
        }
    }

    /// Check whether HP crossed the trigger threshold. Fires the spike (returns
    /// `true`, sets `just_triggered`) the first time `current_hp / max_hp <
    /// trigger_fraction`. No-op if already triggered, `max_hp ≤ 0`, or
    /// disabled.
    pub fn check_and_trigger(&mut self, current_hp: f32, max_hp: f32) -> bool {
        if !self.enabled || self.triggered || max_hp <= 0.0 {
            return false;
        }
        if current_hp / max_hp < self.trigger_fraction {
            self.triggered = true;
            self.timer = self.duration;
            self.just_triggered = true;
            return true;
        }
        false
    }

    /// Allow the spike to fire again. Call when HP rises back above the
    /// threshold or on any appropriate reset event.
    pub fn reset(&mut self) {
        self.triggered = false;
    }

    /// Advance the spike timer; sets `just_ended` when the effect expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_triggered = false;
        self.just_ended = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.just_ended = true;
            }
        }
    }

    /// True while the damage spike is in effect.
    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective outgoing damage. Returns `base * revenge_multiplier` while
    /// active and enabled, `base` otherwise.
    pub fn effective_damage(&self, base: f32) -> f32 {
        if self.is_active() && self.enabled {
            base * self.revenge_multiplier
        } else {
            base
        }
    }

    /// Fraction of the spike duration remaining [1.0 = just fired, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Revenge {
    fn default() -> Self {
        Self::new(2.0, 0.25, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triggers_below_threshold() {
        let mut r = Revenge::new(2.0, 0.25, 5.0);
        assert!(r.check_and_trigger(20.0, 100.0)); // 0.2 < 0.25
        assert!(r.triggered);
        assert!(r.just_triggered);
        assert!(r.is_active());
    }

    #[test]
    fn no_trigger_above_threshold() {
        let mut r = Revenge::new(2.0, 0.25, 5.0);
        assert!(!r.check_and_trigger(30.0, 100.0)); // 0.3 >= 0.25
        assert!(!r.triggered);
    }

    #[test]
    fn no_trigger_at_exact_threshold() {
        let mut r = Revenge::new(2.0, 0.25, 5.0);
        assert!(!r.check_and_trigger(25.0, 100.0)); // 0.25 not < 0.25
        assert!(!r.triggered);
    }

    #[test]
    fn no_retrigger_without_reset() {
        let mut r = Revenge::new(2.0, 0.25, 5.0);
        r.check_and_trigger(20.0, 100.0);
        r.tick(6.0); // expires
        assert!(!r.check_and_trigger(10.0, 100.0));
    }

    #[test]
    fn retrigger_after_reset() {
        let mut r = Revenge::new(2.0, 0.25, 5.0);
        r.check_and_trigger(20.0, 100.0);
        r.tick(6.0);
        r.reset();
        assert!(r.check_and_trigger(15.0, 100.0));
    }

    #[test]
    fn tick_expires_spike() {
        let mut r = Revenge::new(2.0, 0.25, 5.0);
        r.check_and_trigger(20.0, 100.0);
        r.tick(5.1);
        assert!(!r.is_active());
        assert!(r.just_ended);
    }

    #[test]
    fn tick_clears_just_triggered() {
        let mut r = Revenge::new(2.0, 0.25, 5.0);
        r.check_and_trigger(20.0, 100.0);
        r.tick(0.016);
        assert!(!r.just_triggered);
    }

    #[test]
    fn effective_damage_boosted_while_active() {
        let mut r = Revenge::new(2.0, 0.25, 5.0);
        r.check_and_trigger(20.0, 100.0);
        assert!((r.effective_damage(50.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_damage_base_when_inactive() {
        let r = Revenge::new(2.0, 0.25, 5.0);
        assert!((r.effective_damage(50.0) - 50.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut r = Revenge::new(2.0, 0.25, 4.0);
        r.check_and_trigger(20.0, 100.0);
        r.tick(2.0);
        assert!((r.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_no_trigger() {
        let mut r = Revenge::new(2.0, 0.25, 5.0);
        r.enabled = false;
        assert!(!r.check_and_trigger(10.0, 100.0));
    }

    #[test]
    fn disabled_effective_damage_base() {
        let mut r = Revenge::new(2.0, 0.25, 5.0);
        r.check_and_trigger(20.0, 100.0);
        r.enabled = false;
        assert!((r.effective_damage(50.0) - 50.0).abs() < 1e-5);
    }

    #[test]
    fn no_trigger_when_max_hp_zero() {
        let mut r = Revenge::new(2.0, 0.25, 5.0);
        assert!(!r.check_and_trigger(0.0, 0.0));
    }
}
