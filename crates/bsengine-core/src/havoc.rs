use bevy_ecs::prelude::Component;

/// Chaotic-attack modifier: while in havoc, each attack has a `stray_chance`
/// probability of hitting a random nearby target instead of the intended one,
/// AND all attack damage is amplified by `damage_multiplier`.
///
/// `call(duration)` starts or extends the state (high-watermark); sets
/// `just_entered` on the inactive → active transition. `quell()` ends it early.
/// `tick(dt)` counts down and sets `just_exited` on expiry.
///
/// The attack-dispatch system is responsible for rolling against `stray_chance`
/// per attack; this component only tracks state and provides the query helpers
/// `will_stray(roll)` and `effective_damage(base)`.
///
/// Distinct from `Reckless` (trade-off of damage-for-defense), `Rampage`
/// (stacking kill-streak buff), and `Rage` (generic berserker state): Havoc
/// specifically introduces **friendly-fire chaos** — attacks stray to
/// unintended targets while damage is simultaneously amplified.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Havoc {
    pub duration: f32,
    pub timer: f32,
    /// Probability [0.0, 1.0] that each attack strays to a random nearby target.
    pub stray_chance: f32,
    /// Damage multiplier while in havoc. Clamped ≥ 1.0.
    pub damage_multiplier: f32,
    pub just_entered: bool,
    pub just_exited: bool,
    pub enabled: bool,
}

impl Havoc {
    pub fn new(stray_chance: f32, damage_multiplier: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            stray_chance: stray_chance.clamp(0.0, 1.0),
            damage_multiplier: damage_multiplier.max(1.0),
            just_entered: false,
            just_exited: false,
            enabled: true,
        }
    }

    /// Enter or extend the havoc state for `duration` seconds. High-watermark:
    /// only replaces the current timer when `duration > timer`. Sets
    /// `just_entered` on the inactive → active transition. No-op when disabled
    /// or `duration ≤ 0`.
    pub fn call(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_active = self.is_in_havoc();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_entered = true;
            }
        }
    }

    /// End the havoc state early. Sets `just_exited`. No-op when not in havoc.
    pub fn quell(&mut self) {
        if !self.is_in_havoc() {
            return;
        }
        self.timer = 0.0;
        self.duration = 0.0;
        self.just_exited = true;
    }

    /// Advance the havoc timer. Sets `just_exited` when the state expires
    /// naturally. Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_entered = false;
        self.just_exited = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_exited = true;
            }
        }
    }

    pub fn is_in_havoc(&self) -> bool {
        self.timer > 0.0
    }

    /// Returns `true` when the attack-dispatch system should redirect this
    /// attack to a random nearby target. `roll` should be a uniform random
    /// value in [0.0, 1.0] supplied by the caller.
    pub fn will_stray(&self, roll: f32) -> bool {
        self.is_in_havoc() && self.enabled && roll < self.stray_chance
    }

    /// Effective outgoing damage while in havoc and enabled.
    /// Returns `base * damage_multiplier`; returns `base` otherwise.
    pub fn effective_damage(&self, base: f32) -> f32 {
        if self.is_in_havoc() && self.enabled {
            base * self.damage_multiplier
        } else {
            base
        }
    }

    /// Fraction of the havoc duration remaining [1.0 = just entered, 0.0 = exited].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Havoc {
    fn default() -> Self {
        Self::new(0.3, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_enters_havoc() {
        let mut h = Havoc::new(0.3, 1.5);
        h.call(5.0);
        assert!(h.is_in_havoc());
        assert!(h.just_entered);
    }

    #[test]
    fn call_extends_on_longer_duration() {
        let mut h = Havoc::new(0.3, 1.5);
        h.call(3.0);
        h.tick(0.016);
        h.call(8.0);
        assert!((h.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn call_no_extend_on_shorter_duration() {
        let mut h = Havoc::new(0.3, 1.5);
        h.call(8.0);
        h.call(3.0);
        assert!((h.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn just_entered_not_set_on_extend() {
        let mut h = Havoc::new(0.3, 1.5);
        h.call(3.0);
        h.tick(0.016);
        h.call(8.0);
        assert!(!h.just_entered);
    }

    #[test]
    fn quell_ends_havoc() {
        let mut h = Havoc::new(0.3, 1.5);
        h.call(5.0);
        h.quell();
        assert!(!h.is_in_havoc());
        assert!(h.just_exited);
    }

    #[test]
    fn quell_no_op_when_not_active() {
        let mut h = Havoc::new(0.3, 1.5);
        h.quell();
        assert!(!h.just_exited);
    }

    #[test]
    fn tick_expires_havoc() {
        let mut h = Havoc::new(0.3, 1.5);
        h.call(1.0);
        h.tick(1.1);
        assert!(!h.is_in_havoc());
        assert!(h.just_exited);
    }

    #[test]
    fn tick_clears_just_entered() {
        let mut h = Havoc::new(0.3, 1.5);
        h.call(5.0);
        h.tick(0.016);
        assert!(!h.just_entered);
    }

    #[test]
    fn tick_clears_just_exited() {
        let mut h = Havoc::new(0.3, 1.5);
        h.call(0.5);
        h.tick(1.0);
        h.tick(0.016);
        assert!(!h.just_exited);
    }

    #[test]
    fn tick_no_op_when_not_active() {
        let mut h = Havoc::new(0.3, 1.5);
        h.tick(1.0);
        assert!(!h.just_exited);
    }

    #[test]
    fn will_stray_below_threshold() {
        let mut h = Havoc::new(0.5, 1.5);
        h.call(5.0);
        assert!(h.will_stray(0.49));
    }

    #[test]
    fn will_stray_at_threshold_false() {
        let mut h = Havoc::new(0.5, 1.5);
        h.call(5.0);
        assert!(!h.will_stray(0.5));
    }

    #[test]
    fn will_stray_false_when_not_in_havoc() {
        let h = Havoc::new(0.9, 1.5);
        assert!(!h.will_stray(0.0));
    }

    #[test]
    fn effective_damage_amplified_in_havoc() {
        let mut h = Havoc::new(0.3, 2.0);
        h.call(5.0);
        assert!((h.effective_damage(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_damage_base_when_not_in_havoc() {
        let h = Havoc::new(0.3, 2.0);
        assert!((h.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut h = Havoc::new(0.3, 1.5);
        h.call(4.0);
        h.tick(2.0);
        assert!((h.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_zero_when_not_active() {
        let h = Havoc::new(0.3, 1.5);
        assert!((h.remaining_fraction()).abs() < 1e-5);
    }

    #[test]
    fn disabled_call_no_op() {
        let mut h = Havoc::new(0.3, 1.5);
        h.enabled = false;
        h.call(5.0);
        assert!(!h.is_in_havoc());
    }

    #[test]
    fn disabled_will_stray_false() {
        let mut h = Havoc::new(0.9, 1.5);
        h.call(5.0);
        h.enabled = false;
        assert!(!h.will_stray(0.0));
    }

    #[test]
    fn disabled_effective_damage_base() {
        let mut h = Havoc::new(0.3, 2.0);
        h.call(5.0);
        h.enabled = false;
        assert!((h.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }
}
