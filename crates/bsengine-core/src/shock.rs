use bevy_ecs::prelude::Component;

/// Electric-discharge debuff that deals periodic lightning damage and carries
/// a per-second chance to interrupt the target's current action.
///
/// `tick(dt)` returns the damage dealt this frame (`damage_per_second * dt`)
/// while the effect is active. `check_interrupt(rng, dt)` tests whether the
/// electric spasm interrupts an action this frame.
///
/// `apply(duration)` uses high-watermark. `clear()` removes the effect early.
///
/// Distinct from `Burn` (fire DoT, no interrupts), `Stun` (total CC, no
/// ongoing damage), and `Jolt` (single-frame impulse): Shock is a sustained
/// debuff combining low ongoing damage with probabilistic action interrupts.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Shock {
    pub duration: f32,
    pub timer: f32,
    /// Lightning damage dealt per second while shocked.
    pub damage_per_second: f32,
    /// Probability per second of interrupting the entity's current action.
    pub interrupt_chance: f32,
    pub just_shocked: bool,
    pub just_discharged: bool,
    pub enabled: bool,
}

impl Shock {
    pub fn new(damage_per_second: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            damage_per_second: damage_per_second.max(0.0),
            interrupt_chance: 0.0,
            just_shocked: false,
            just_discharged: false,
            enabled: true,
        }
    }

    pub fn with_interrupt_chance(mut self, chance: f32) -> Self {
        self.interrupt_chance = chance.clamp(0.0, 1.0);
        self
    }

    /// Apply or extend the shock for `duration` seconds. High-watermark:
    /// only replaces the current timer if the new duration is longer.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_shocked = true;
            }
        }
    }

    /// Clear the shock immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_discharged = true;
        }
    }

    /// Advance the timer. Returns damage dealt this frame; sets `just_discharged`
    /// when the effect expires.
    pub fn tick(&mut self, dt: f32) -> f32 {
        self.just_shocked = false;
        self.just_discharged = false;

        if self.timer > 0.0 {
            let damage = self.damage_per_second * dt;
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_discharged = true;
            }
            return damage;
        }
        0.0
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Returns `true` if a random spasm interrupts the entity's action this
    /// frame. `rng_value` is a uniform random in `[0.0, 1.0)`. The per-frame
    /// probability is approximated as `interrupt_chance * dt`.
    pub fn check_interrupt(&self, rng_value: f32, dt: f32) -> bool {
        self.is_active() && rng_value < (self.interrupt_chance * dt)
    }

    /// Fraction of the shock duration remaining [1.0 = just applied, 0.0 = discharged].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Shock {
    fn default() -> Self {
        Self::new(5.0).with_interrupt_chance(0.15)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_shock() {
        let mut s = Shock::new(10.0);
        s.apply(3.0);
        assert!(s.is_active());
        assert!(s.just_shocked);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut s = Shock::new(10.0);
        s.apply(2.0);
        s.tick(0.016);
        s.apply(5.0);
        assert!((s.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut s = Shock::new(10.0);
        s.apply(5.0);
        s.apply(2.0);
        assert!((s.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_returns_damage() {
        let mut s = Shock::new(10.0);
        s.apply(5.0);
        let dmg = s.tick(0.1);
        assert!((dmg - 1.0).abs() < 1e-5); // 10 * 0.1
    }

    #[test]
    fn tick_returns_zero_when_inactive() {
        let mut s = Shock::new(10.0);
        let dmg = s.tick(0.1);
        assert!((dmg - 0.0).abs() < 1e-5);
    }

    #[test]
    fn tick_expires_shock() {
        let mut s = Shock::new(10.0);
        s.apply(1.0);
        s.tick(1.1);
        assert!(!s.is_active());
        assert!(s.just_discharged);
    }

    #[test]
    fn clear_ends_shock() {
        let mut s = Shock::new(10.0);
        s.apply(5.0);
        s.clear();
        assert!(!s.is_active());
        assert!(s.just_discharged);
    }

    #[test]
    fn check_interrupt_true_when_rng_below_threshold() {
        let s = Shock::new(10.0).with_interrupt_chance(1.0);
        let mut shock = s;
        shock.apply(5.0);
        // chance=1.0, dt=1.0 → threshold=1.0; rng=0.5 < 1.0 → true
        assert!(shock.check_interrupt(0.5, 1.0));
    }

    #[test]
    fn check_interrupt_false_when_rng_above_threshold() {
        let mut s = Shock::new(10.0).with_interrupt_chance(0.1);
        s.apply(5.0);
        // chance=0.1, dt=0.016 → threshold=0.0016; rng=0.5 > threshold → false
        assert!(!s.check_interrupt(0.5, 0.016));
    }

    #[test]
    fn check_interrupt_false_when_inactive() {
        let s = Shock::new(10.0).with_interrupt_chance(1.0);
        assert!(!s.check_interrupt(0.0, 1.0));
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut s = Shock::new(10.0);
        s.apply(2.0);
        s.tick(1.0);
        assert!((s.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut s = Shock::new(10.0);
        s.enabled = false;
        s.apply(5.0);
        assert!(!s.is_active());
    }

    #[test]
    fn tick_clears_just_shocked() {
        let mut s = Shock::new(10.0);
        s.apply(3.0);
        s.tick(0.016);
        assert!(!s.just_shocked);
    }
}
