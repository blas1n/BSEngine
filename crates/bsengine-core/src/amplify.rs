use bevy_ecs::prelude::Component;

/// Buff that multiplies the power of abilities and effects the entity outputs.
///
/// While amplified, the ability system calls `effective_power(base)` to scale
/// up healing amounts, damage values, buff durations, or any other output
/// quantity before it is applied. A power_multiplier of 1.5 means all outputs
/// are 50% stronger.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_faded` on expiry. `clear()` removes the buff early.
///
/// Distinct from `Boost` (stat-specific increases), `Surge` (short burst of
/// raw damage), and `Haste` (speed multiplier): Amplify is a generic output
/// multiplier that scales whatever the entity produces — heals, buffs, debuffs,
/// or damage — making it ideal for support/caster archetypes.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Amplify {
    pub duration: f32,
    pub timer: f32,
    /// Multiplier applied to all ability outputs while amplified (>= 1.0).
    pub power_multiplier: f32,
    pub just_amplified: bool,
    pub just_faded: bool,
    pub enabled: bool,
}

impl Amplify {
    pub fn new(power_multiplier: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            power_multiplier: power_multiplier.max(1.0),
            just_amplified: false,
            just_faded: false,
            enabled: true,
        }
    }

    /// Apply or extend the amplify buff for `duration` seconds. High-watermark:
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
                self.just_amplified = true;
            }
        }
    }

    /// Remove the amplify buff immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_faded = true;
        }
    }

    /// Advance the timer; sets `just_faded` when the buff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_amplified = false;
        self.just_faded = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_faded = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Scale an ability output by the amplification factor.
    /// Returns `base * power_multiplier` while active, `base` otherwise.
    pub fn effective_power(&self, base: f32) -> f32 {
        if self.is_active() {
            base * self.power_multiplier
        } else {
            base
        }
    }

    /// Fraction of the amplify duration remaining [1.0 = just applied, 0.0 = faded].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Amplify {
    fn default() -> Self {
        Self::new(1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_amplify() {
        let mut a = Amplify::new(1.5);
        a.apply(3.0);
        assert!(a.is_active());
        assert!(a.just_amplified);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut a = Amplify::new(1.5);
        a.apply(2.0);
        a.tick(0.016);
        a.apply(5.0);
        assert!((a.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut a = Amplify::new(1.5);
        a.apply(5.0);
        a.apply(2.0);
        assert!((a.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_amplify() {
        let mut a = Amplify::new(1.5);
        a.apply(1.0);
        a.tick(1.1);
        assert!(!a.is_active());
        assert!(a.just_faded);
    }

    #[test]
    fn clear_ends_early() {
        let mut a = Amplify::new(1.5);
        a.apply(5.0);
        a.clear();
        assert!(!a.is_active());
        assert!(a.just_faded);
    }

    #[test]
    fn effective_power_while_active() {
        let mut a = Amplify::new(2.0);
        a.apply(3.0);
        assert!((a.effective_power(50.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_power_passthrough_when_inactive() {
        let a = Amplify::new(2.0);
        assert!((a.effective_power(50.0) - 50.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut a = Amplify::new(1.5);
        a.apply(2.0);
        a.tick(1.0);
        assert!((a.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut a = Amplify::new(1.5);
        a.enabled = false;
        a.apply(5.0);
        assert!(!a.is_active());
    }

    #[test]
    fn tick_clears_just_amplified() {
        let mut a = Amplify::new(1.5);
        a.apply(3.0);
        a.tick(0.016);
        assert!(!a.just_amplified);
    }

    #[test]
    fn power_multiplier_clamped_to_min_one() {
        let a = Amplify::new(0.5);
        assert!((a.power_multiplier - 1.0).abs() < 1e-5);
    }
}
