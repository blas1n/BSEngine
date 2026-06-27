use bevy_ecs::prelude::Component;

/// Debuff that reduces the effectiveness of an entity's active shields.
///
/// While active, the shield system should multiply incoming shield absorption
/// by `shield_multiplier()` (< 1.0). At `reduction_fraction == 1.0` shields
/// are completely bypassed; at `0.5` they absorb at half effectiveness.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_recovered` when the debuff expires.
///
/// Distinct from `Disarm` (prevents weapon use), `Nullify` (blocks buff/debuff
/// application), and `Weaken` (outgoing damage): ShieldBreak specifically
/// targets the entity's own shield absorption, modelling armour-pierce and
/// shield-shred abilities.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct ShieldBreak {
    pub duration: f32,
    pub timer: f32,
    /// Fraction of shield effectiveness removed [0.0 = no effect, 1.0 = shields nullified].
    pub reduction_fraction: f32,
    pub just_broken: bool,
    pub just_recovered: bool,
    pub enabled: bool,
}

impl ShieldBreak {
    pub fn new(reduction_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            reduction_fraction: reduction_fraction.clamp(0.0, 1.0),
            just_broken: false,
            just_recovered: false,
            enabled: true,
        }
    }

    /// Apply or extend the shield-break for `duration` seconds. High-watermark:
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
                self.just_broken = true;
            }
        }
    }

    /// Remove the debuff immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_recovered = true;
        }
    }

    /// Advance the timer; sets `just_recovered` when the debuff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_broken = false;
        self.just_recovered = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_recovered = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Fraction of shield absorption remaining while broken; `1.0` when inactive.
    pub fn shield_multiplier(&self) -> f32 {
        if self.is_active() {
            1.0 - self.reduction_fraction
        } else {
            1.0
        }
    }

    /// Fraction of the debuff duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for ShieldBreak {
    fn default() -> Self {
        Self::new(0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_shield_break() {
        let mut s = ShieldBreak::new(0.5);
        s.apply(3.0);
        assert!(s.is_active());
        assert!(s.just_broken);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut s = ShieldBreak::new(0.5);
        s.apply(2.0);
        s.tick(0.016);
        s.apply(5.0);
        assert!((s.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut s = ShieldBreak::new(0.5);
        s.apply(5.0);
        s.apply(2.0);
        assert!((s.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_debuff() {
        let mut s = ShieldBreak::new(0.5);
        s.apply(1.0);
        s.tick(1.1);
        assert!(!s.is_active());
        assert!(s.just_recovered);
    }

    #[test]
    fn clear_ends_early() {
        let mut s = ShieldBreak::new(0.5);
        s.apply(5.0);
        s.clear();
        assert!(!s.is_active());
        assert!(s.just_recovered);
    }

    #[test]
    fn shield_multiplier_while_active() {
        let mut s = ShieldBreak::new(0.4);
        s.apply(3.0);
        assert!((s.shield_multiplier() - 0.6).abs() < 1e-5);
    }

    #[test]
    fn shield_multiplier_when_inactive() {
        let s = ShieldBreak::new(0.4);
        assert!((s.shield_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn shield_multiplier_full_break() {
        let mut s = ShieldBreak::new(1.0);
        s.apply(3.0);
        assert!((s.shield_multiplier() - 0.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut s = ShieldBreak::new(0.5);
        s.apply(2.0);
        s.tick(1.0);
        assert!((s.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut s = ShieldBreak::new(0.5);
        s.enabled = false;
        s.apply(5.0);
        assert!(!s.is_active());
    }

    #[test]
    fn tick_clears_just_broken() {
        let mut s = ShieldBreak::new(0.5);
        s.apply(3.0);
        s.tick(0.016);
        assert!(!s.just_broken);
    }
}
